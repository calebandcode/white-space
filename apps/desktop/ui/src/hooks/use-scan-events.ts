import { useEffect } from "react"

import { useFolderStore } from "@/store/folder-store"

interface ScanProgressPayload {
  scanned: number
  skipped: number
  errors: number
  path_sample?: string | null
}

interface ScanFinishedPayload {
  scanned: number
  skipped: number
  errors: number
  error_messages: string[]
  started_at?: string | null
  finished_at?: string | null
}

interface ScanErrorPayload {
  message: string
}

export function useScanEvents() {
  const handleProgress = useFolderStore((state) => state.handleScanProgress)
  const handleDone = useFolderStore((state) => state.handleScanDone)
  const handleQueued = useFolderStore((state) => state.handleScanQueued)
  const handleError = useFolderStore((state) => state.handleScanError)
  const refreshStatus = useFolderStore((state) => state.refreshScanStatus)
  const loadCandidates = useFolderStore((state) => state.loadCandidates)
  const loadDir = useFolderStore((state) => state.loadDir)
  const scanStatus = useFolderStore((state) => state.scan.status)

  useEffect(() => {
    let isCancelled = false
    const unsubs: Array<() => void> = []

    ;(async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event")
        const offProgress = await listen<ScanProgressPayload>("scan://progress", (event) => {
          handleProgress(event.payload)
        })
        if (isCancelled) {
          offProgress()
        } else {
          unsubs.push(offProgress)
        }

        const offDone = await listen<ScanFinishedPayload>("scan://done", (event) => {
          void handleDone(event.payload)
        })
        if (isCancelled) {
          offDone()
        } else {
          unsubs.push(offDone)
        }

        const offError = await listen<ScanErrorPayload>("scan://error", (event) => {
          handleError(event.payload)
        })
        if (isCancelled) {
          offError()
        } else {
          unsubs.push(offError)
        }

        const offQueued = await listen<{ roots: number }>("scan://queued", (event) => {
          handleQueued(event.payload)
        })
        if (isCancelled) {
          offQueued()
        } else {
          unsubs.push(offQueued)
        }

        // Roots changed -> refresh folders, gauge, candidates
        const offRoots = await listen<{ count: number }>("roots://changed", async () => {
          try {
            await useFolderStore.getState().loadFolders()
            await useFolderStore.getState().loadGauge()
            await useFolderStore.getState().loadCandidates()
          } catch (e) {
            // ignore
          }
        })
        if (isCancelled) {
          offRoots()
        } else {
          unsubs.push(offRoots)
        }
      } catch (error) {
        console.error("Failed to register scan event listeners", error)
      }
    })()

    void refreshStatus()

    return () => {
      isCancelled = true
      for (const unsub of unsubs) {
        try {
          unsub()
        } catch (error) {
          console.error("Failed to remove scan listener", error)
        }
      }
    }
  }, [handleDone, handleError, handleProgress, handleQueued, refreshStatus])

  useEffect(() => {
    if (scanStatus !== "running") {
      return
    }

    const interval = setInterval(() => {
      void refreshStatus()
    }, 1500)

    return () => clearInterval(interval)
  }, [scanStatus, refreshStatus])

  // Fallback: periodic refresh when capabilities block event.listen
  useEffect(() => {
    let cancelled = false

    const tick = async () => {
      try {
        await refreshStatus()
        const folderId = useFolderStore.getState().selectedFolderId
        if (folderId) {
          await loadDir(folderId)
          await loadCandidates()
        }
      } catch (e) {
        // ignore
      }
    }

    const interval = setInterval(() => {
      if (document.visibilityState === "visible" && !cancelled) {
        void tick()
      }
    }, 5000)

    const onFocus = () => { void tick() }
    window.addEventListener("focus", onFocus)
    document.addEventListener("visibilitychange", onFocus)

    return () => {
      cancelled = true
      clearInterval(interval)
      window.removeEventListener("focus", onFocus)
      document.removeEventListener("visibilitychange", onFocus)
    }
  }, [loadCandidates, loadDir, refreshStatus])
}
