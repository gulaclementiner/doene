# Docker lathund (cheat sheet)

Compose file: /workspace/docker-compose.yml — service `vibe`, container `ai-vibe`.
Run all commands from `/workspace`.

## Start
    docker compose up -d                                        # start the container
    docker compose exec -w /workspace/projects/döne vibe bash   # shell into it (project dir)

## Stop
    docker compose stop    # stop only, keep container
    docker compose down    # stop + remove container

## Useful
    docker compose logs -f     # follow logs
    docker compose ps          # status
    docker compose restart     # restart

Notes:
- Container runs `sleep infinity` with `restart: unless-stopped` → it survives host reboots; use stop/down to really shut it off.
- Project files are a volume mount (`.:/workspace`) → nothing is lost on down.
