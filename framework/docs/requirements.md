# Projekty

## IPEL GameDev - gdd.ics.upjs.sk

### Requirements

```
Service:        IPEL GameDev — multiplayer web game platform
Deployment:     1× Docker container on Linux (amd64)
Repository:     github.com/matusem/ipel-gamedev (framework/)
CPU:            4 cores recommended (minimum 2)
RAM:            8 GB recommended (minimum 4 GB)
Disk:           40 GB SSD with persistent volumes (minimum 20 GB)
OS:             Debian 14
Network:        HTTPS (443), WebSocket-capable reverse proxy
Host packages:  docker.io, ca-certificates, curl
Database:       SQLite (embedded, no separate DB server)
Scaling:        Single instance only
Users:          ~200 total, ~50 concurrent games, ~300 peak WebSockets
Backup:         Daily — SQLite volume + /app/games volume
URL:            https://[hostname.example.uni.sk]/
Contact:        [dev team contact]
```

## IPEL Rust - uwu.ics.upjs.sk

## IHRA modernizacia - ihra.ics.upjs.sk

