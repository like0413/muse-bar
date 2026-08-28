use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use wasapi::{AudioCaptureClient, AudioClient, Direction, SampleType, StreamMode, WaveFormat};

use crate::{
    background_worker::{join_with_timeout, WORKER_SHUTDOWN_TIMEOUT},
    media::MediaVolumeIdentity,
    volume::ApplicationPeakMeter,
};

const SAMPLE_RATE: u32 = 44_100;
const CHANNEL_COUNT: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const BYTES_PER_FRAME: usize =
    CHANNEL_COUNT as usize * (BITS_PER_SAMPLE as usize / u8::BITS as usize);
const FFT_SIZE: usize = 1_024;
const BAND_COUNT: usize = 16;
const CAPTURE_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CAPTURE_RETRY_DELAY: Duration = Duration::from_millis(1500);
const SPECTRUM_FRAME_EVENT: &str = "spectrum-frame";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpectrumFrame {
    session_key: u64,
    levels: [f32; BAND_COUNT],
}

struct ActiveSpectrum {
    session_key: u64,
    stop_requested: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

/// 保证任意时刻至多存在一个当前播放器频谱采集线程。
#[derive(Default)]
pub(crate) struct SpectrumManager {
    active: Mutex<Option<ActiveSpectrum>>,
}

impl SpectrumManager {
    pub(crate) fn start(
        &self,
        app: AppHandle,
        session_key: u64,
        process_id: u32,
        identity: MediaVolumeIdentity,
        frame_rate: u8,
    ) -> Result<(), String> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| "无法保存频谱采集状态：状态锁已损坏".to_owned())?;
        stop_active(active.take());
        let stop_requested = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop_requested);
        let worker = thread::Builder::new()
            .name("muse-bar-spectrum".to_owned())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    let Err(error) = run_capture(
                        &app,
                        session_key,
                        process_id,
                        &identity,
                        frame_rate,
                        &worker_stop,
                    ) else {
                        break;
                    };
                    log::warn!("当前应用频谱采集已停止：{error}");
                    emit_silence(&app, session_key);
                    let retry_started_at = Instant::now();
                    while retry_started_at.elapsed() < CAPTURE_RETRY_DELAY
                        && !worker_stop.load(Ordering::Acquire)
                    {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            })
            .map_err(|error| format!("无法启动频谱采集线程：{error}"))?;
        *active = Some(ActiveSpectrum {
            session_key,
            stop_requested,
            worker,
        });
        Ok(())
    }

    pub(crate) fn stop(&self) {
        let active = self.active.lock().ok().and_then(|mut active| active.take());
        stop_active(active);
    }

    pub(crate) fn stop_session(&self, expected_session_key: u64) {
        let active = self.active.lock().ok().and_then(|mut active| {
            active
                .as_ref()
                .is_some_and(|active| active.session_key == expected_session_key)
                .then(|| active.take())
                .flatten()
        });
        stop_active(active);
    }

    pub(crate) fn request_shutdown(&self) {
        self.stop();
    }
}

fn stop_active(active: Option<ActiveSpectrum>) {
    if let Some(active) = active {
        active.stop_requested.store(true, Ordering::Release);
        join_with_timeout(active.worker, "频谱采集", WORKER_SHUTDOWN_TIMEOUT);
    }
}

impl Drop for SpectrumManager {
    fn drop(&mut self) {
        if let Ok(active) = self.active.get_mut() {
            if let Some(active) = active.take() {
                active.stop_requested.store(true, Ordering::Release);
            }
        }
    }
}

fn run_capture(
    app: &AppHandle,
    session_key: u64,
    process_id: u32,
    identity: &MediaVolumeIdentity,
    frame_rate: u8,
    stop_requested: &AtomicBool,
) -> Result<(), String> {
    wasapi::initialize_mta()
        .ok()
        .map_err(|error| format!("无法初始化频谱 COM 线程：{error}"))?;
    let _com_guard = ComGuard;
    let peak_meter = ApplicationPeakMeter::open(identity)?;
    let format = WaveFormat::new(
        BITS_PER_SAMPLE as usize,
        BITS_PER_SAMPLE as usize,
        &SampleType::Int,
        SAMPLE_RATE as usize,
        CHANNEL_COUNT as usize,
        None,
    );
    let mut audio_client = AudioClient::new_application_loopback_client(process_id, true)
        .map_err(|error| format!("无法激活进程回环音频接口：{error}"))?;
    audio_client
        .initialize_client(
            &format,
            &Direction::Capture,
            &StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: 0,
            },
        )
        .map_err(|error| format!("无法初始化进程回环音频流：{error}"))?;
    let capture_client = audio_client
        .get_audiocaptureclient()
        .map_err(|error| format!("无法取得频谱采集接口：{error}"))?;
    audio_client
        .start_stream()
        .map_err(|error| format!("无法启动频谱音频流：{error}"))?;

    let capture_result = capture_loop(
        app,
        session_key,
        &capture_client,
        peak_meter.as_ref(),
        frame_rate,
        stop_requested,
    );
    if let Err(error) = audio_client.stop_stream() {
        log::debug!("频谱音频流停止失败：{error}");
    }
    capture_result
}

fn capture_loop(
    app: &AppHandle,
    session_key: u64,
    capture: &AudioCaptureClient,
    peak_meter: Option<&ApplicationPeakMeter>,
    frame_rate: u8,
    stop_requested: &AtomicBool,
) -> Result<(), String> {
    let mut samples = VecDeque::with_capacity(FFT_SIZE);
    let mut packet_buffer = Vec::new();
    let mut analyzer = SpectrumAnalyzer::new();
    let mut meter_visualizer = MeterVisualizer::default();
    let frame_interval = Duration::from_secs_f32(1.0 / f32::from(frame_rate));
    let mut last_frame_at = Instant::now()
        .checked_sub(frame_interval)
        .unwrap_or_else(Instant::now);
    let mut first_frame_source_logged = false;

    while !stop_requested.load(Ordering::Acquire) {
        let received_audio = drain_packets(capture, &mut samples, &mut packet_buffer)?;
        if last_frame_at.elapsed() >= frame_interval {
            let fft_levels = (received_audio && samples.len() >= FFT_SIZE)
                .then(|| analyzer.analyze(samples.make_contiguous()));
            let fft_peak = fft_levels
                .as_ref()
                .map(|levels| levels.iter().copied().fold(0.0_f32, f32::max))
                .unwrap_or_default();
            let meter_peak = peak_meter.map_or(0.0, ApplicationPeakMeter::peak);
            let (levels, source) = if fft_peak > 0.004 || meter_peak <= 0.004 {
                (fft_levels.unwrap_or([0.0; BAND_COUNT]), "fft")
            } else {
                (meter_visualizer.analyze(meter_peak), "meter")
            };
            if !first_frame_source_logged && levels.iter().any(|level| *level > 0.004) {
                log::info!("频谱可视帧已生成：session_key={session_key}, source={source}");
                first_frame_source_logged = true;
            }
            app.emit_to(
                "bar",
                SPECTRUM_FRAME_EVENT,
                SpectrumFrame {
                    session_key,
                    levels,
                },
            )
            .map_err(|error| format!("无法发送频谱帧：{error}"))?;
            last_frame_at = Instant::now();
        }
        if !received_audio {
            thread::sleep(CAPTURE_POLL_INTERVAL);
        }
    }
    Ok(())
}

fn drain_packets(
    capture: &AudioCaptureClient,
    samples: &mut VecDeque<f32>,
    packet_buffer: &mut Vec<u8>,
) -> Result<bool, String> {
    let mut received_audio = false;
    loop {
        let packet_frames = capture
            .get_next_packet_size()
            .map_err(|error| format!("无法读取频谱音频包大小：{error}"))?
            .unwrap_or_default();
        if packet_frames == 0 {
            return Ok(received_audio);
        }
        received_audio = true;

        let required_bytes = packet_frames as usize * BYTES_PER_FRAME;
        packet_buffer.resize(required_bytes, 0);
        let (frame_count, buffer_info) = capture
            .read_from_device(packet_buffer)
            .map_err(|error| format!("无法读取频谱音频数据：{error}"))?;
        let data = &packet_buffer[..frame_count as usize * BYTES_PER_FRAME];
        if buffer_info.flags.silent {
            push_samples(samples, (0..frame_count).map(|_| 0.0));
        } else {
            push_samples(
                samples,
                data.chunks_exact(BYTES_PER_FRAME).map(|frame| {
                    let left = i16::from_le_bytes(frame[0..2].try_into().unwrap_or_default());
                    let right = i16::from_le_bytes(frame[2..4].try_into().unwrap_or_default());
                    (f32::from(left) + f32::from(right)) / 65_536.0
                }),
            );
        }
    }
}

fn push_samples(samples: &mut VecDeque<f32>, incoming: impl Iterator<Item = f32>) {
    for sample in incoming {
        if samples.len() == FFT_SIZE {
            samples.pop_front();
        }
        samples.push_back(sample);
    }
}

struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex32>,
    window: Vec<f32>,
    band_ranges: Vec<(usize, usize)>,
    smoothed: [f32; BAND_COUNT],
}

#[derive(Default)]
struct MeterVisualizer {
    phase: f32,
    smoothed: [f32; BAND_COUNT],
}

impl MeterVisualizer {
    fn analyze(&mut self, peak: f32) -> [f32; BAND_COUNT] {
        self.phase = (self.phase + 0.19) % std::f32::consts::TAU;
        let amplitude = (peak.clamp(0.0, 1.0).powf(0.55) * 1.35).min(1.0);
        for (band, level) in self.smoothed.iter_mut().enumerate() {
            let position = band as f32 / (BAND_COUNT - 1) as f32;
            let movement = (self.phase * (1.0 + position * 0.35) + band as f32 * 1.41)
                .sin()
                .abs();
            let contour = 1.0 - position * 0.22;
            let target = amplitude * contour * (0.42 + movement * 0.58);
            let smoothing = if target > *level { 0.55 } else { 0.2 };
            *level += (target - *level) * smoothing;
        }
        self.smoothed
    }
}

impl SpectrumAnalyzer {
    fn new() -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let denominator = (FFT_SIZE - 1) as f32;
        let window = (0..FFT_SIZE)
            .map(|index| {
                let phase = std::f32::consts::TAU * index as f32 / denominator;
                0.5 * (1.0 - phase.cos())
            })
            .collect();
        let band_ranges = logarithmic_band_ranges();
        Self {
            fft,
            buffer: vec![Complex32::default(); FFT_SIZE],
            window,
            band_ranges,
            smoothed: [0.0; BAND_COUNT],
        }
    }

    fn analyze(&mut self, samples: &[f32]) -> [f32; BAND_COUNT] {
        for ((slot, sample), window) in self.buffer.iter_mut().zip(samples).zip(&self.window) {
            *slot = Complex32::new(*sample * window, 0.0);
        }
        self.fft.process(&mut self.buffer);

        for (band, &(start, end)) in self.band_ranges.iter().enumerate() {
            let peak_squared = self.buffer[start..end]
                .iter()
                .map(Complex32::norm_sqr)
                .fold(0.0_f32, f32::max);
            let peak = peak_squared.sqrt() / (FFT_SIZE as f32 * 0.5);
            let decibels = 20.0 * peak.max(1.0e-6).log10();
            let level = ((decibels + 68.0) / 58.0).clamp(0.0, 1.0).powf(0.72);
            let smoothing = if level > self.smoothed[band] {
                0.62
            } else {
                0.22
            };
            self.smoothed[band] += (level - self.smoothed[band]) * smoothing;
        }
        self.smoothed
    }
}

fn logarithmic_band_ranges() -> Vec<(usize, usize)> {
    let minimum_hz = 55.0_f32;
    let maximum_hz = 16_000.0_f32;
    let ratio = maximum_hz / minimum_hz;
    (0..BAND_COUNT)
        .map(|band| {
            let start_hz = minimum_hz * ratio.powf(band as f32 / BAND_COUNT as f32);
            let end_hz = minimum_hz * ratio.powf((band + 1) as f32 / BAND_COUNT as f32);
            let start = frequency_bin(start_hz).max(1);
            let end = frequency_bin(end_hz).max(start + 1).min(FFT_SIZE / 2);
            (start, end)
        })
        .collect()
}

fn frequency_bin(frequency: f32) -> usize {
    (frequency * FFT_SIZE as f32 / SAMPLE_RATE as f32).floor() as usize
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}

fn emit_silence(app: &AppHandle, session_key: u64) {
    let _ = app.emit_to(
        "bar",
        SPECTRUM_FRAME_EVENT,
        SpectrumFrame {
            session_key,
            levels: [0.0; BAND_COUNT],
        },
    );
}
