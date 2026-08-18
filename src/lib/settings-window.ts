import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

const SETTINGS_WINDOW_LABEL = 'settings'

let settingsWindowRequest: Promise<void> | undefined

/** 创建设置窗口，并在原生窗口可获得焦点后结束等待。 */
function createSettingsWindow(): Promise<void> {
  return new Promise((resolve, reject) => {
    const settingsWindow = new WebviewWindow(SETTINGS_WINDOW_LABEL, {
      url: 'index.html#/settings',
      title: 'Muse Bar Settings',
      width: 960,
      height: 680,
      minWidth: 720,
      minHeight: 520,
      center: true,
    })

    // Tauri 异步创建原生窗口，因此必须等创建完成后再设置焦点。
    void settingsWindow.once('tauri://created', async () => {
      try {
        await settingsWindow.setFocus()
        resolve()
      } catch (error) {
        reject(error)
      }
    })

    void settingsWindow.once<string>('tauri://error', ({ payload }) => {
      reject(new Error(payload))
    })
  })
}

/** 优先显示已有设置窗口；窗口已关闭时才重新创建。 */
async function showOrCreateSettingsWindow(): Promise<void> {
  const settingsWindow = await WebviewWindow.getByLabel(SETTINGS_WINDOW_LABEL)

  if (settingsWindow) {
    await settingsWindow.show()
    await settingsWindow.setFocus()
    return
  }

  await createSettingsWindow()
}

/**
 * 打开设置窗口，并合并短时间内的重复请求。
 *
 * 每个窗口都会运行一份前端，但只有 Bar 页面会导入本模块。复用正在执行的 Promise，
 * 可以防止连续点击在查询窗口标签时产生竞争，进而尝试创建两个同名原生窗口。
 */
export function openSettingsWindow(): Promise<void> {
  if (!settingsWindowRequest) {
    settingsWindowRequest = showOrCreateSettingsWindow().finally(() => {
      settingsWindowRequest = undefined
    })
  }

  return settingsWindowRequest
}
