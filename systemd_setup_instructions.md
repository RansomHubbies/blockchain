# Setting Up Blockchain Testnet as a Systemd Service

Follow these steps to set up your testnet as a systemd service that will automatically start after VM reboots:

## 1. Copy the service script

Make the service script executable and copy it to your blockchain directory:

```bash
chmod +x start_testnet.sh
```

## 2. Install the systemd service

Copy the service file to the systemd directory:

```bash
sudo cp blockchain.service /etc/systemd/system/
```

## 3. Enable and start the service

```bash
# Reload systemd to recognize the new service
sudo systemctl daemon-reload

# Enable the service to start on boot
sudo systemctl enable blockchain.service

# Start the service now
sudo systemctl start blockchain.service
```

## 4. Check service status

```bash
# Check if the service is running properly
sudo systemctl status blockchain.service
```

## Managing the service

```bash
# To stop the service
sudo systemctl stop blockchain.service

# To restart the service
sudo systemctl restart blockchain.service

# To view service logs
sudo journalctl -u blockchain.service -f
```

## Notes

-   The service is configured to restart automatically if it crashes or if the VM reboots
-   Logs are sent to the systemd journal and can be viewed with journalctl
-   The service runs as the iiitd user, which should have all necessary permissions

## Troubleshooting

If the service fails to start:

1. Check the logs: `sudo journalctl -u blockchain.service -e`
2. Verify the path to your project directory in the start script (`/home/iiitd/blockchain`)
3. Make sure the iiitd user has permissions to run the cargo commands
4. Ensure that all dependencies are installed for the iiitd user
