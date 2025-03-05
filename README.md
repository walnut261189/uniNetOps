# uniNetOps
A universal network operation automater toolkit supporting templatising daily automation activities.
Features:
1. Universal template for network device os upgrade

## Notes:
The code has been generalized to serve as a template for any vendor type and network device. It uses an abstraction via the NetworkDevice trait, which can be implemented for different vendor types. You can customize it further for specific device requirements or protocols (e.g., gRPC).

### Configuration Info
The code loads parameters from a config.json file, allowing dynamic runtime updates without restarting the application. The configuration file is parsed into a shared, mutable structure that can be updated in real-time using a watcher mechanism.

### Configuration Parameters
1. base_url: The API endpoint of the Cisco device.
2. token: The authentication token for the Cisco device's API.
3. os_file_path: Path to the Cisco OS upgrade binary file on your system.
#### Usage:
Place the config.json file in the root directory of your application.
The application will load these parameters dynamically at runtime.

## Usage instructions for the project
Run the following command to fetch and install all dependencies
```
  cargo build
```

```
{
  "server": {
    "port": 3000
  },
  "rabbitmq": {
    "host": "amqp://localhost",
    "queue": "message_queue",
    "reconnectInterval": 5000
  },
  "logging": {
    "logDirectory": "./logs",
    "maxSize": "10m",
    "maxFiles": "1d"
  },
  "rateLimiter": {
    "windowMs": 60000,
    "maxRequests": 100
  },
  "auth": {
    "tokenSecret": "your-secure-token-secret"
  }
}

```










