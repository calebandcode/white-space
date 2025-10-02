# White Space

White Space is an AI-powered, local-first file decluttering desktop application built with **Rust** (Tauri 2), **React**, **Vite**, and **TypeScript**. It helps users organize their desktop by detecting duplicate files, managing oversized or outdated files, and making suggestions for tidying up.

## Features

- **Tauri 2**: A modern desktop app framework for building cross-platform apps with web technologies.
- **React 18**: The latest version of React, utilizing hooks and modern patterns.
- **Vite**: A fast and highly efficient build tool and development server.
- **TypeScript**: Strict type-checking for better code quality and developer experience.
- **Tailwind CSS**: A utility-first CSS framework for building responsive designs.
- **shadcn/ui**: Beautiful, accessible UI components for a polished interface.
- **Zustand**: Lightweight and flexible state management.
- **File System Capabilities**: Secure file operations with least privilege access.
- **AI-Powered Features**: On-device AI features such as duplicate detection (size → partial SHA-1 → full SHA-1) and content classification (screenshots, installers, large downloads).
- **Staging & Undo**: Archive-first with a 7-day cooling-off period and one-click Undo.

## Project Structure

```plaintext
white-space/
├── src-tauri/                 # Tauri backend
│   ├── src/                   # Rust source code
│   ├── capabilities/          # Tauri capabilities
│   │   └── fs.json            # File system permissions
│   ├── migrations/            # Database migrations
│   ├── Cargo.toml             # Rust dependencies
│   ├── tauri.conf.json        # Tauri configuration
│   └── build.rs               # Build script
├── apps/
│   └── desktop/
│       └── ui/                # React frontend
│           ├── src/
│           │   ├── components/  # Reusable UI components
│           │   ├── hooks/       # Custom React hooks
│           │   ├── store/       # State management
│           │   ├── pages/       # Page components
│           │   └── lib/         # Utility functions
│           ├── package.json     # Frontend dependencies
│           ├── vite.config.ts   # Vite configuration
│           ├── tailwind.config.js # Tailwind configuration
│           └── tsconfig.json   # TypeScript configuration
└── package.json               # Root workspace configuration
```

## Getting Started

### Prerequisites

- **Node.js** 18+
- **Rust** 1.70+
- **Tauri CLI** 2.0+

### Installation

1. Install frontend dependencies:

   ```bash
   npm install
   ```

2. Install Tauri CLI globally (if not already installed):

   ```bash
   npm install -g @tauri-apps/cli
   ```

### Development

To run the development environment:

```bash
cargo tauri dev
```

This command will:

- Start the **Vite** dev server at `http://localhost:1420`
- Launch the Tauri desktop app
- Enable hot-reloading for both frontend and backend

### Building

To build the production version of the app:

```bash
npm run tauri:build
```

This will generate the installer for your platform (Windows, macOS, or Linux).

## Capabilities

White Space ensures file system access with the least privilege necessary. The application includes capabilities like:

- Read, write, create, remove, and rename files and directories
- Monitor file paths (watched folders)
- Detect file changes and duplicates

The permissions are controlled via `src-tauri/capabilities/fs.json`.

## Backend Flow

- `scan_status` returns `{ state, scanned, skipped, errors, started_at, finished_at, roots, current_path, last_error }` for lightweight UI polling.
- Progress events: `scan://progress` stream incremental counts with a sample path.
- Completion events: `scan://done` includes final counts and any collected error messages.
- Error events: `scan://error` surfaces individual scan issues for UI notifications.

```rust
const SCAN_PROGRESS_EVENT: &str = "scan://progress";
const SCAN_DONE_EVENT: &str = "scan://done";
const SCAN_ERROR_EVENT: &str = "scan://error";
```

## Screenshots

Here are some visuals of the application in action:

1. **App in Action**: ![App in Action](https://github.com/calebandcode/white-space/blob/main/assets/readme-img.png)
2. **Folder Selection**: ![Folder Selection](https://github.com/calebandcode/white-space/blob/main/assets/folder-selection.png)

## Tech Stack

- **Frontend**: React 18, TypeScript, Vite, Tailwind CSS, shadcn/ui
- **Backend**: Rust, Tauri 2
- **State Management**: Zustand
- **Database**: SQLite (with migrations)
- **Build Tools**: Vite, Tauri CLI
- **AI Tools**: MiniLM, Tesseract OCR

## Scripts

- `npm run dev` - Start Vite dev server
- `npm run build` - Build the frontend
- `cargo tauri dev` - Start Tauri development server (front-end + back-end)
- `cargo tauri build` - Build Tauri application
