import * as React from "react"

type Theme = "dark" | "light" | "system"

type ThemeProviderProps = {
  children: React.ReactNode
  defaultTheme?: Theme
  storageKey?: string
}

type ThemeProviderState = {
  theme: Theme
  setTheme: (theme: Theme) => void
}

const initialState: ThemeProviderState = {
  theme: "system",
  setTheme: () => null,
}

const ThemeProviderContext = React.createContext<ThemeProviderState>(initialState)

export function ThemeProvider({
  children,
  defaultTheme = "system",
  storageKey = "vite-ui-theme",
  ...props
}: ThemeProviderProps) {
  const getStoredTheme = () => {
    if (typeof window === "undefined") return defaultTheme
    return (localStorage.getItem(storageKey) as Theme) || defaultTheme
  }

  const [theme, setTheme] = React.useState<Theme>(getStoredTheme)
  const [resolvedTheme, setResolvedTheme] = React.useState<"light" | "dark">(() => {
    if (typeof window === "undefined") return "light"
    const initial = getStoredTheme()
    if (initial === "system") {
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
    }

    return initial
  })

  React.useEffect(() => {
    if (typeof window === "undefined") return

    const media = window.matchMedia("(prefers-color-scheme: dark)")
    const systemTheme = () => (media.matches ? "dark" : "light")

    const updateResolvedTheme = () => {
      setResolvedTheme(theme === "system" ? systemTheme() : theme)
    }

    updateResolvedTheme()

    if (theme !== "system") {
      return
    }

    media.addEventListener("change", updateResolvedTheme)
    return () => media.removeEventListener("change", updateResolvedTheme)
  }, [theme])

  React.useEffect(() => {
    if (typeof window === "undefined") return

    const root = window.document.documentElement
    root.classList.remove("light", "dark")
    root.classList.add(resolvedTheme)

    const tauriPresent = "__TAURI_IPC__" in window || "__TAURI_INTERNALS__" in window

    if (!tauriPresent) {
      return
    }

    const updateTauriTitlebar = async () => {
      try {
        const windowApi = await import("@tauri-apps/api/window")
        const appWindow = (windowApi as any).appWindow
        const WindowTheme = (windowApi as any).WindowTheme

        if (appWindow && typeof appWindow.setTheme === "function" && WindowTheme) {
          await appWindow.setTheme(
            resolvedTheme === "dark" ? WindowTheme.Dark : WindowTheme.Light
          )
        }
      } catch (error) {
        console.debug("Unable to sync Tauri window theme", error)
      }
    }

    updateTauriTitlebar()
  }, [resolvedTheme])

  const value = React.useMemo(
    () => ({
      theme,
      setTheme: (nextTheme: Theme) => {
        if (typeof window !== "undefined") {
          localStorage.setItem(storageKey, nextTheme)
        }
        setTheme(nextTheme)
      },
    }),
    [theme, storageKey]
  )

  return (
    <ThemeProviderContext.Provider {...props} value={value}>
      {children}
    </ThemeProviderContext.Provider>
  )
}

export const useTheme = () => {
  const context = React.useContext(ThemeProviderContext)

  if (context === undefined)
    throw new Error("useTheme must be used within a ThemeProvider")

  return context
}
