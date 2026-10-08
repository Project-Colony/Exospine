Exospine App Icon - How to Replace
===================================

The current icon.ico is a placeholder. To create a proper icon:

1. Create a 256x256+ PNG with your design
   - Suggested: a distinctive spine/vertebra motif in teal (#00bcd4) on a dark background
   - This fits the Exospine brand identity

2. Run the Tauri icon generator:
   cargo tauri icon <path-to-your-256x256-png>

3. This command generates all required sizes into this directory:
   - icon.ico  (Windows)
   - icon.icns (macOS)
   - Multiple PNGs at various resolutions (Linux)

4. The icons are automatically picked up by the Tauri build process.
