# Bitcoin Core macOS File Descriptor Issue - Fix

## Problem
Bitcoin Core on macOS may fail to start with the error:
```
Error: Not enough file descriptors available. -1 available, 160 required.
```

This is a known issue with Bitcoin Core v29+ on macOS systems.

## Solution

### Required Fix (Must be done with admin privileges)
1. Open a new terminal and run with sudo:
```bash
sudo launchctl limit maxfiles 65536 200000
```

2. **IMPORTANT**: After running the command, you must:
   - Close ALL terminal windows
   - Open a fresh terminal
   - Or restart your system

3. Verify the change took effect:
```bash
launchctl limit maxfiles
# Should show: maxfiles    65536    200000

ulimit -n
# Should show: 65536 or higher
```

### If the above doesn't work
Some macOS versions require editing system files:
1. Create/edit `/Library/LaunchDaemons/limit.maxfiles.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>limit.maxfiles</string>
    <key>ProgramArguments</key>
    <array>
        <string>launchctl</string>
        <string>limit</string>
        <string>maxfiles</string>
        <string>65536</string>
        <string>200000</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
```

2. Load it:
```bash
sudo launchctl load -w /Library/LaunchDaemons/limit.maxfiles.plist
```

3. Restart your system

### Alternative: Use Docker
If the issue persists, consider running Bitcoin Core in Docker:
```bash
docker run -d --name bitcoin-regtest \
  -p 18443:18443 \
  -v bitcoin-data:/bitcoin \
  ruimarinho/bitcoin-core \
  -regtest=1 \
  -server=1 \
  -rpcuser=bitstable \
  -rpcpassword=password \
  -rpcallowip=0.0.0.0/0 \
  -rpcbind=0.0.0.0
```

## Verification
After applying the fix, verify Bitcoin Core can start:
```bash
./scripts/start_regtest.sh
```

## Notes
- This is a known issue with Bitcoin Core on macOS related to how the OS handles file descriptors
- The start_regtest.sh script has been updated to handle this automatically
- If you continue to have issues, check the debug log: `tail -f ~/.bitcoin/regtest/debug.log`