# White Space

A Tauri 2 + React/Vite + TypeScript desktop application.

## Features

- **Tauri 2**: Modern desktop app framework
- **React 18**: Latest React with hooks and modern patterns
- **Vite**: Fast build tool and dev server
- **TypeScript**: Strict type checking for better code quality
- **Tailwind CSS**: Utility-first CSS framework
- **shadcn/ui**: Beautiful, accessible UI components
- **ESLint + Prettier**: Code linting and formatting
- **Zustand**: Lightweight state management
- **File System Capabilities**: Secure file operations with least privilege

## Project Structure

```
white-space/
├── src-tauri/                 # Tauri backend
│   ├── src/                   # Rust source code
│   ├── capabilities/          # Tauri capabilities
│   │   └── fs.json           # File system permissions
│   ├── migrations/           # Database migrations
│   ├── Cargo.toml            # Rust dependencies
│   ├── tauri.conf.json       # Tauri configuration
│   └── build.rs              # Build script
├── apps/
│   └── desktop/
│       └── ui/               # React frontend
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
└── package.json              # Root workspace configuration
```

## Getting Started

### Prerequisites

- Node.js 18+
- Rust 1.70+
- Tauri CLI 2.0+

### Installation

1. Install dependencies:

```bash
npm install
```

2. Install Tauri CLI (if not already installed):

```bash
npm install -g @tauri-apps/cli
```

### Development

Start the development server:

```bash
npm run tauri:dev
```

This will:

- Start the Vite dev server on `http://localhost:1420`
- Launch the Tauri desktop app
- Enable hot reload for both frontend and backend changes

### Building

Build the application for production:

```bash
npm run tauri:build
```

## Capabilities

The application includes a file system capability configuration (`src-tauri/capabilities/fs.json`) that provides least-privilege access to:

- Read files and directories
- Write files
- Create directories
- Remove files and directories
- Rename files
- Copy files
- Check file existence

## Scripts

- `npm run dev` - Start Vite dev server
- `npm run build` - Build frontend
- `npm run tauri:dev` - Start Tauri development
- `npm run tauri:build` - Build Tauri application
- `npm run lint` - Run ESLint
- `npm run preview` - Preview production build

## Tech Stack

- **Frontend**: React 18, TypeScript, Vite, Tailwind CSS, shadcn/ui
- **Backend**: Rust, Tauri 2
- **State Management**: Zustand
- **Code Quality**: ESLint, Prettier, TypeScript strict mode
- **Build Tools**: Vite, Tauri CLI



## Backend Flow

- `scan_status` returns `{ state, scanned, skipped, errors, started_at, finished_at, roots, current_path, last_error }` for lightweight UI polling.
- Progress events: `scan://progress` stream incremental counts with a sample path.
- Completion events: `scan://done` include final counts plus any collected error messages.
- Error events: `scan://error` surface individual scan issues so the UI can raise toast notifications.

```rust
const SCAN_PROGRESS_EVENT: &str = "scan://progress";
const SCAN_DONE_EVENT: &str = "scan://done";
const SCAN_ERROR_EVENT: &str = "scan://error";
```




