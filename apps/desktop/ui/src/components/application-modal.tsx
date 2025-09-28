"use client"

import * as React from "react"
import {
  HelpCircle,
  Home,
  Loader2,
  PlayCircle,
  Archive,
  Search,
  Settings,
} from "lucide-react"

import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from "@/components/ui/sidebar"
import { useFolderStore } from "@/store/folder-store"
import { Link, useLocation } from "react-router-dom"

const data = {
  nav: [
    { name: "Home", icon: Home, path: "/" },
    { name: "Archive", icon: Archive, path: "/archive" },
  ],
}

function ApplicationModal({ children }: React.PropsWithChildren) {
  const [open, setOpen] = React.useState(true)
  const location = useLocation()
  const startScan = useFolderStore((state) => state.startScan)
  const scanStatus = useFolderStore((state) => state.scan.status)
  const isScanning = scanStatus === "running" || scanStatus === "queued"

  const handleStartScan = React.useCallback(() => {
    if (isScanning) return
    void startScan()
  }, [isScanning, startScan])

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        className="fixed inset-0 h-full w-full max-w-none overflow-hidden rounded-none border-0 bg-background p-0 sm:max-w-none translate-x-0 translate-y-0"
        showCloseButton={false}
      >
        <SidebarProvider className="flex h-full w-full items-stretch">
          <Sidebar collapsible="none" className="hidden h-full md:flex">
            <SidebarContent className="border-r">
              <SidebarGroup>
                <SidebarGroupContent className="gap-4">
                  <SidebarMenu className="mt-10">
                    {data.nav.map((item) => {
                      const isActive = item.path ? location.pathname === item.path : false
                      return (
                        <SidebarMenuItem key={item.name}>
                          <SidebarMenuButton asChild isActive={isActive}>
                            <Link to={item.path}>
                              <item.icon />
                              <span>{item.name}</span>
                            </Link>
                          </SidebarMenuButton>
                        </SidebarMenuItem>
                      )
                    })}
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            </SidebarContent>
          </Sidebar>
          <main className="flex h-full flex-1 flex-col overflow-hidden bg-background">
            <header
              className="flex flex-col gap-4 border-b border-border/70 px-6 pb-4 pt-8"
              data-tauri-drag-region="true"
            >
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div
                  className="group flex w-full items-center gap-2 rounded-full border border-border/60 bg-background/80 px-3 py-2 shadow-sm transition-colors focus-within:border-primary/50 focus-within:ring-2 focus-within:ring-primary/10 md:max-w-xl"
                  data-tauri-drag-region="false"
                >
                  <Search className="size-5 bg-transparent text-muted-foreground transition-colors group-focus-within:text-primary" />
                  <Input
                    placeholder="Search..."
                    aria-label="Search folders"
                    className="h-8 w-full flex-1 border-none bg-transparent px-0 text-sm text-foreground placeholder:text-muted-foreground/70 focus-visible:border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
                  />
                </div>
                <div
                  className="flex items-center gap-2 self-end md:self-auto"
                  data-tauri-drag-region="false"
                >
                  <Button
                    variant="ghost"
                    size="icon"
                    className="rounded-full"
                    aria-label={isScanning ? "Scan in progress" : "Start scan"}
                    onClick={handleStartScan}
                    disabled={isScanning}
                  >
                    {isScanning ? (
                      <Loader2 className="size-5 animate-spin" />
                    ) : (
                      <PlayCircle className="size-5" />
                    )}
                  </Button>
                  <Button variant="ghost" size="icon" className="rounded-full">
                    <HelpCircle className="size-5" />
                  </Button>
                  <Button variant="ghost" size="icon" className="rounded-full">
                    <Settings className="size-5" />
                  </Button>
                </div>
              </div>
            </header>
            <div className="flex flex-1 flex-col overflow-y-auto px-6 pb-6">
              {children ? (
                children
              ) : (
                <div className="mt-10 flex justify-center">
                  <p className="text-sm text-muted-foreground">No content available</p>
                </div>
              )}
            </div>
          </main>
        </SidebarProvider>
      </DialogContent>
    </Dialog>
  )
}

export default ApplicationModal
