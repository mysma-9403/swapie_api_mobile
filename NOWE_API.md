# Swapie Backend (Rust) - Instrukcja wdrożenia

## Wymagania

- Serwer Linux (Ubuntu 22.04+)
- Docker + Docker Compose
- Nginx
- Domena `api.swapie.pl` skierowana na IP serwera
- Konto GitHub z dostępem do repo `mysma-9403/swapie_api_mobile`

---

## 1. Konfiguracja GitHub Actions Secrets

Wejdź na: `https://github.com/mysma-9403/swapie_api_mobile/settings/secrets/actions`

Dodaj 3 sekrety:

| Secret | Wartość |
|--------|---------|
| `SERVER_HOST` | IP serwera (np. `123.45.67.89`) |
| `SERVER_USER` | Nazwa użytkownika SSH (np. `root` lub `swapie`) |
| `SERVER_SSH_KEY` | Cały prywatny klucz SSH (zawartość `~/.ssh/id_rsa` lub `id_ed25519`) |

### Jak wygenerować klucz SSH (jeśli nie masz):

```bash
# Na swoim komputerze
ssh-keygen -t ed25519 -C "deploy@swapie" -f ~/.ssh/swapie_deploy

# Skopiuj klucz publiczny na serwer
ssh-copy-id -i ~/.ssh/swapie_deploy.pub root@<IP_SERWERA>

# Zawartość klucza prywatnego wklej do SERVER_SSH_KEY:
cat ~/.ssh/swapie_deploy
```

---

## 2. Jednorazowy setup serwera

### 2.1 Zaloguj się na serwer

```bash
ssh root@<IP_SERWERA>
```

### 2.2 Zainstaluj Docker, Nginx, Certbot

```bash
apt-get update && apt-get upgrade -y
apt-get install -y docker.io docker-compose-plugin nginx certbot python3-certbot-nginx ufw fail2ban curl
systemctl enable docker
systemctl start docker
```

### 2.3 Firewall

```bash
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable
```

### 2.4 Utwórz katalog aplikacji

```bash
mkdir -p /opt/swapie
cd /opt/swapie
```

### 2.5 Skopiuj pliki z repo (z lokalnej maszyny)

```bash
# Na swoim komputerze, z katalogu projektu:
scp docker-compose.yml root@<IP_SERWERA>:/opt/swapie/
scp .env.example root@<IP_SERWERA>:/opt/swapie/.env
scp deploy/nginx.conf root@<IP_SERWERA>:/etc/nginx/sites-available/api.swapie.pl
scp deploy/proxy_params root@<IP_SERWERA>:/etc/nginx/proxy_params
```

### 2.6 Skonfiguruj .env (na serwerze)

```bash
nano /opt/swapie/.env
```

Uzupełnij produkcyjne wartości:

```env
APP_ENV=production
APP_DEBUG=false
APP_HOST=0.0.0.0
APP_PORT=8080
APP_URL=https://api.swapie.pl
APP_WORKERS=4

DATABASE_URL=mysql://swapie:TWOJE_HASLO@mysql:3306/swapie
DATABASE_MAX_CONNECTIONS=50

JWT_SECRET=WYGENERUJ_LOSOWY_KLUCZ_64_ZNAKI

STRIPE_SECRET_KEY=sk_live_...
STRIPE_PUBLISHABLE_KEY=pk_live_...
STRIPE_WEBHOOK_SECRET=whsec_...

RABBITMQ_HOST=rabbitmq
RABBITMQ_PORT=5672
RABBITMQ_USER=swapie
RABBITMQ_PASSWORD=SILNE_HASLO_RABBIT

DB_ROOT_PASSWORD=SILNE_HASLO_ROOT
DB_DATABASE=swapie
DB_USERNAME=swapie
DB_PASSWORD=TWOJE_HASLO

INPOST_API_BASE_URL=https://api.inpost-group.com/points
INPOST_API_TOKEN=...

BOOK_API_URL=https://library.lagano.pl

FCM_SERVER_KEY=...
FCM_PROJECT_ID=...

S3_ENDPOINT=https://ams3.digitaloceanspaces.com
S3_BUCKET=swapie-media
S3_REGION=ams3
S3_ACCESS_KEY=...
S3_SECRET_KEY=...
S3_URL=https://swapie-media.ams3.digitaloceanspaces.com

SMTP_HOST=...
SMTP_PORT=587
SMTP_USERNAME=...
SMTP_PASSWORD=...
SMTP_FROM_EMAIL=noreply@swapie.pl
SMTP_FROM_NAME=Swapie

RUST_LOG=swapie_backend=info,actix_web=warn
```

Wygeneruj losowy JWT_SECRET:
```bash
openssl rand -hex 32
```

### 2.7 SSL (Let's Encrypt)

```bash
# Tymczasowa konfiguracja nginx żeby certbot mógł zweryfikować domenę
cat > /etc/nginx/sites-available/api.swapie.pl << 'TMPEOF'
server {
    listen 80;
    server_name api.swapie.pl;
    location / { return 200 'ok'; }
}
TMPEOF

ln -sf /etc/nginx/sites-available/api.swapie.pl /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default
nginx -t && systemctl restart nginx

# Wygeneruj certyfikat
certbot certonly --nginx -d api.swapie.pl --non-interactive --agree-tos -m admin@swapie.pl

# Teraz wgraj właściwą konfigurację nginx (tę skopiowaną wcześniej)
# Plik /etc/nginx/sites-available/api.swapie.pl powinien być już skopiowany z deploy/nginx.conf

# Jeśli nie, skopiuj ponownie:
# scp deploy/nginx.conf root@<IP>:/etc/nginx/sites-available/api.swapie.pl

nginx -t && systemctl restart nginx

# Auto-renewal
echo "0 3 * * * certbot renew --quiet --post-hook 'systemctl reload nginx'" | crontab -
```

### 2.8 Zaloguj się do GitHub Container Registry (na serwerze)

```bash
# Wygeneruj Personal Access Token na GitHub:
# https://github.com/settings/tokens -> Generate new token (classic)
# Zaznacz: read:packages

echo "TWOJ_GITHUB_TOKEN" | docker login ghcr.io -u mysma-9403 --password-stdin
```

### 2.9 Pierwsze uruchomienie

```bash
cd /opt/swapie

# Pobierz obraz (po pierwszym PUSH na main i build w CI)
docker pull ghcr.io/mysma-9403/swapie_api_mobile:latest

# Uruchom wszystko
docker compose up -d

# Sprawdź czy działa
docker compose ps
docker compose logs api --tail 50

# Test endpointu
curl -s https://api.swapie.pl/api/v1/genres | head
```

---

## 3. Jak działa CI/CD

```
Push na main
    │
    ▼
GitHub Actions (.github/workflows/deploy.yml)
    │
    ├── 1. Buduje Docker image (multi-stage, ~10-15 MB binary)
    ├── 2. Pushuje do ghcr.io/mysma-9403/swapie_api_mobile:latest
    │
    ▼
Deploy via SSH
    │
    ├── 3. docker pull najnowszy obraz
    ├── 4. docker compose up -d (restart kontenerów)
    └── 5. docker image prune (cleanup starych obrazów)
```

**Każdy push na `main` automatycznie deployuje na serwer.**

CI (`.github/workflows/ci.yml`) odpala się też na Pull Requestach - sprawdza kompilację i clippy.

---

## 4. Kontenery (docker compose)

| Kontener | Port | Opis |
|----------|------|------|
| `swapie-api` | 8080 | Serwer HTTP (Actix-web) + Scheduler |
| `swapie-worker-default` | - | Worker: sync paczkomatów, ogólne |
| `swapie-worker-emails` | - | Worker: emaile, etykiety |
| `swapie-worker-sms` | - | Worker: SMS-y |
| `swapie-worker-shipments` | - | Worker: tworzenie przesyłek (InPost/Orlen) |
| `swapie-worker-notifications` | - | Worker: push powiadomienia |
| `swapie-mysql` | 3306 | MySQL 8.0 |
| `swapie-rabbitmq` | 5672, 15672 | RabbitMQ + panel zarządzania |

---

## 5. Przydatne komendy (na serwerze)

```bash
cd /opt/swapie

# Status kontenerów
docker compose ps

# Logi API
docker compose logs api --tail 100 -f

# Logi konkretnego workera
docker compose logs worker-shipments --tail 50 -f

# Restart API (bez downtime)
docker compose restart api

# Restart wszystkiego
docker compose down && docker compose up -d

# Ręczny pull i deploy
docker pull ghcr.io/mysma-9403/swapie_api_mobile:latest
docker compose up -d

# Wejście do kontenera
docker compose exec api sh

# Sprawdzenie bazy
docker compose exec mysql mysql -u swapie -p swapie

# Panel RabbitMQ
# http://<IP_SERWERA>:15672 (guest/guest lub skonfigurowane hasło)
# UWAGA: na produkcji ogranicz port 15672 w firewall lub wyłącz

# Backup bazy
docker compose exec mysql mysqldump -u root -p swapie > backup_$(date +%Y%m%d).sql
```

---

## 6. Monitoring i debugowanie

```bash
# Sprawdź health
curl -s https://api.swapie.pl/api/v1/genres

# Sprawdź logi nginx
tail -f /var/log/nginx/api.swapie.pl.error.log

# Sprawdź zużycie zasobów
docker stats

# Sprawdź kolejki RabbitMQ
docker compose exec rabbitmq rabbitmqctl list_queues name messages consumers
```

---

## 7. Aktualizacja .env na produkcji

```bash
# Edytuj
nano /opt/swapie/.env

# Restart żeby pobrać nowe zmienne
docker compose down && docker compose up -d
```

---

## 8. Skalowanie

### Więcej workerów tego samego typu:
```bash
docker compose up -d --scale worker-emails=3 --scale worker-shipments=2
```

### Więcej instancji API (load balancing):
Zmień w `docker-compose.yml`:
```yaml
api:
  deploy:
    replicas: 3
```
I w nginx upstream:
```nginx
upstream swapie_backend {
    server 127.0.0.1:8080;
    server 127.0.0.1:8081;
    server 127.0.0.1:8082;
    keepalive 32;
}
```

### Zewnętrzna baza danych:
Zmień `DATABASE_URL` w `.env` na adres zewnętrznej bazy (RDS, PlanetScale) i usuń serwis `mysql` z `docker-compose.yml`.

---

## 9. Rollback

Jeśli nowa wersja nie działa:

```bash
# Sprawdź dostępne obrazy
docker images ghcr.io/mysma-9403/swapie_api_mobile

# Przywróć konkretną wersję (SHA z git commita)
docker pull ghcr.io/mysma-9403/swapie_api_mobile:<SHA>
docker compose up -d
```

---

## 10. Bezpieczeństwo produkcyjne

- [ ] Zmień domyślne hasła (MySQL root, RabbitMQ)
- [ ] Wygeneruj silny JWT_SECRET (`openssl rand -hex 32`)
- [ ] Ogranicz port 15672 (RabbitMQ management) - tylko z VPN/localhost
- [ ] Ogranicz port 3306 (MySQL) - tylko z kontenera
- [ ] Włącz fail2ban
- [ ] Regularnie aktualizuj system (`apt upgrade`)
- [ ] Ustaw backup bazy (cron + mysqldump + upload na S3)
- [ ] Monitoruj logi pod kątem błędów
