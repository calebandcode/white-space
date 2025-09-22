import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification'

import { formatBytes } from '@/hooks/useGauge'

export async function notifySweepReady(bytes: number) {
  if (typeof window === 'undefined') return
  try {
    let granted = await isPermissionGranted()
    if (!granted) {
      granted = (await requestPermission()) === 'granted'
    }
    if (!granted) return

    await sendNotification({
      title: 'White Space',
      body: `You have ${formatBytes(bytes)} ready to sweep. Review & delete?`,
    })
  } catch (error) {
    console.error('Failed to send notification', error)
  }
}
