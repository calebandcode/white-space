import { fireEvent, render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { FolderItem } from "../FolderItem"
import type { WatchedFolder } from "@/types/folders"

const baseFolder: WatchedFolder = {
  id: "1",
  name: "Screenshots",
  path: "C:/Users/me/Pictures/Screenshots",
  platformStyle: "win",
  isAccessible: true,
}

describe("FolderItem", () => {
  it("renders the folder name and truncation styles", () => {
    render(<FolderItem folder={{ ...baseFolder, name: "A Very Long Folder Name That Should Truncate" }} />)

    const item = screen.getByRole("listitem")
    const label = screen.getByText("A Very Long Folder Name That Should Truncate")
    const nameWrapper = label.parentElement

    expect(item).toHaveAttribute("title", "A Very Long Folder Name That Should Truncate")
    expect(nameWrapper).not.toBeNull()
    if (!nameWrapper) {
      throw new Error("name wrapper should exist")
    }
    expect(nameWrapper).toHaveClass("truncate", { exact: false })
  })

  it("invokes onOpen when double-clicked", async () => {
    const user = userEvent.setup()
    const handleOpen = vi.fn()

    render(<FolderItem folder={baseFolder} onOpen={handleOpen} />)

    const item = screen.getByRole("listitem")
    await user.dblClick(item)

    expect(handleOpen).toHaveBeenCalledWith("1")
  })

  it("supports inline rename via keyboard", async () => {
    const user = userEvent.setup()
    const handleRename = vi.fn()

    render(<FolderItem folder={baseFolder} onRename={handleRename} />)

    const item = screen.getByRole("listitem")
    item.focus()
    fireEvent.keyDown(item, { key: "F2" })

    const input = await screen.findByLabelText("Rename folder")
    await user.clear(input)
    await user.type(input, "Edited")
    expect((input as HTMLInputElement).value).toBe("Edited")
    await user.keyboard("{Enter}")

    expect(handleRename).toHaveBeenCalledWith("1", "Edited")
  })

  it("shows context menu actions and triggers callbacks", async () => {
    const user = userEvent.setup()
    const handleRemove = vi.fn()

    render(<FolderItem folder={baseFolder} onRemove={handleRemove} />)

    const item = screen.getByRole("listitem")
    fireEvent.contextMenu(item)

    const remove = await screen.findByText("Remove from watched")
    await user.click(remove)

    expect(handleRemove).toHaveBeenCalledWith("1")
  })

  it("exposes aria-selected when selected", () => {
    render(<FolderItem folder={baseFolder} selected />)

    const item = screen.getByRole("listitem")

    expect(item).toHaveAttribute("aria-selected", "true")
    expect(item.getAttribute("data-selected")).toBe("true")
  })
})







