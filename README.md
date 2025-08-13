# Screen Ghost - Monitor Demo

A Tauri + React application that demonstrates real-time monitor capture and display functionality.

## Features

- **Monitor Detection**: Automatically detects and displays all available monitors
- **Monitor Selection**: Click on any monitor to select it for capture
- **Real-time Image Display**: Shows live screenshots from the selected monitor
- **Modern UI**: Clean, responsive interface with dark mode support

## How to Use

1. **Launch the Application**: Run `npm run tauri dev` to start the development server
2. **View Available Monitors**: The app will display all detected monitors with their specifications
3. **Select a Monitor**: Click on any monitor card to select it for capture
4. **View Live Images**: Once selected, the app will start receiving and displaying live screenshots from the monitor

## Technical Details

### Frontend (React + TypeScript)
- **Monitor Display**: Grid layout showing all available monitors with position, size, and scale information
- **Image Processing**: Converts BGRA image data from backend to RGBA for canvas display
- **Event Listening**: Listens for `image` events from the Tauri backend
- **Responsive Design**: Works on different screen sizes with mobile support

### Backend (Rust + Tauri)
- **Monitor Detection**: Uses Windows API to enumerate all available monitors
- **Screen Capture**: Captures screenshots every 3 seconds using DirectX Desktop Duplication
- **Event Emission**: Emits captured images to the frontend via Tauri events
- **State Management**: Maintains selected monitor state across the application

## Development

### Prerequisites
- Node.js 18+
- Rust 1.70+
- Windows 10/11 (for monitor capture functionality)

### Setup
```bash
# Install dependencies
npm install

# Start development server
npm run tauri dev

# Build for production
npm run tauri build
```

### Project Structure
```
src/
├── App.tsx          # Main React component
├── App.css          # Styles for the demo interface
└── main.tsx         # React entry point

src-tauri/
├── src/
│   ├── api/
│   │   ├── command.rs    # Tauri commands for frontend
│   │   └── emitter.rs    # Event emission utilities
│   ├── monitor/
│   │   ├── mod.rs        # Monitor detection and management
│   │   └── monitor.rs    # Screen capture implementation
│   └── system/
│       └── monitoring/   # Background monitoring system
└── Cargo.toml
```

## API Commands

- `get_monitors()`: Returns list of all available monitors
- `set_working_monitor(monitor)`: Sets the monitor to capture
- `start_monitoring()`: Starts the background capture process

## Events

- `image`: Emitted when a new screenshot is captured, contains image data in BGRA format

## License

MIT License


### tips
本地安装了llvm, opencv