let shellModulePromise: Promise<{ open: (path: string) => Promise<void> } | null> | null = null

async function loadShellModule() {
  if (shellModulePromise) return shellModulePromise

  shellModulePromise = (async () => {
    if (typeof window === "undefined") return null
    const isTauri = "__TAURI_INTERNALS__" in window || "__TAURI_IPC__" in window
    if (!isTauri) return null

    try {
      const module = await import("@tauri-apps/plugin-shell")
      return module
    } catch (error) {
      console.warn("Failed to load @tauri-apps/plugin-shell; falling back to no-op", error)
      return null
    }
  })()

  return shellModulePromise
}

async function openWithFallback(path: string) {
  if (typeof window !== "undefined" && window.open && path.startsWith("http")) {
    window.open(path, "_blank")
    return true
  }
  return false
}

export async function openInOS(path: string) {
  if (!path) return
  const shell = await loadShellModule()
  if (shell) {
    try {
      await shell.open(path)
      return
    } catch (error) {
      console.error("Failed to open path in OS", error)
    }
  }

  if (!(await openWithFallback(path))) {
    console.warn("openInOS fallback could not handle path", path)
  }
}

export async function revealInOS(path: string) {
  if (!path) return
  const shell = await loadShellModule()
  if (shell) {
    try {
      await shell.open(path)
      return
    } catch (error) {
      console.error("Failed to reveal path in OS", error)
    }
  }

  if (!(await openWithFallback(path))) {
    console.warn("revealInOS fallback could not handle path", path)
  }
}
