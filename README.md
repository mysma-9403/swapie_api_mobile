# Swapie Backend (Rust)

Book & Board Game Trading Platform API — rewritten from Laravel/PHP to Rust with Actix-web.

## Tech Stack

- **Framework**: Actix-web 4 (lightweight, high-performance)
- **Database**: MySQL (SQLx async driver)
- **Auth**: JWT (jsonwebtoken) with Bearer tokens
- **Payments**: Stripe (direct API via reqwest)
- **Storage**: S3-compatible (DigitalOcean Spaces)
- **Push Notifications**: Firebase Cloud Messaging (FCM v1)
- **Password Hashing**: Argon2
- **Rate Limiting**: Governor (token bucket)
- **i18n**: Custom JSON-based translations (EN, PL)
- **Logging**: tracing + tracing-subscriber

## Project Structure

```
swapieBackend/
├── Cargo.toml              # Dependencies & build config
├── .env.example            # All environment variables
├── locales/                # Translation files
│   ├── en.json
│   └── pl.json
├── migrations/             # MySQL schema
│   └── 001_initial_schema.sql
└── src/
    ├── main.rs             # Entry point, server setup
    ├── config/             # App & DB configuration
    ├── models/             # Database models (14 files, 34+ structs)
    ├── handlers/           # API handlers/controllers (17 files)
    ├── services/           # Business logic (13 services)
    ├── middleware/          # Auth JWT & rate limiting
    ├── routes/             # Route configuration
    ├── dto/                # Request/response DTOs
    ├── errors/             # Error types & handling
    ├── i18n/               # Internationalization
    └── utils/              # Helpers (geo, validation, etc.)
```

## Quick Start (Development)

### Prerequisites

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- MySQL 8.0+
- Redis (optional, for future caching)

### Setup

```bash
# 1. Clone and enter project
cd swapieBackend

# 2. Copy and configure environment
cp .env.example .env
# Edit .env with your database credentials, Stripe keys, etc.

# 3. Create database
mysql -u root -e "CREATE DATABASE swapie CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"

# 4. Run migrations
mysql -u root swapie < migrations/001_initial_schema.sql
# Or set RUN_MIGRATIONS=true in .env to auto-run on startup

# 5. Build and run
cargo run

# Server starts at http://0.0.0.0:8080
```

### Development with hot reload

```bash
cargo install cargo-watch
cargo watch -x run
```

## Production Deployment

### 1. Build Optimized Binary

```bash
# Release build with LTO (Link-Time Optimization)
cargo build --release

# Binary at: target/release/swapie_backend (~10-15 MB)
```

The release profile in `Cargo.toml` enables:
- `opt-level = 3` — maximum optimization
- `lto = true` — link-time optimization for smaller binary
- `codegen-units = 1` — better optimization, slower compile
- `strip = true` — remove debug symbols

### 2. Environment Variables

Set all variables from `.env.example` in your production environment. Critical ones:

```bash
APP_ENV=production
APP_DEBUG=false
APP_URL=https://api.swapie.app
APP_WORKERS=8                    # Match your CPU cores
DATABASE_URL=mysql://user:pass@db-host:3306/swapie
DATABASE_MAX_CONNECTIONS=50      # Tune based on load
JWT_SECRET=<strong-random-256bit-key>
STRIPE_SECRET_KEY=sk_live_...
STRIPE_WEBHOOK_SECRET=whsec_...
RUST_LOG=swapie_backend=info,actix_web=warn
```

### 3. Systemd Service (Linux)

```ini
# /etc/systemd/system/swapie-backend.service
[Unit]
Description=Swapie Backend API
After=network.target mysql.service

[Service]
Type=simple
User=swapie
Group=swapie
WorkingDirectory=/opt/swapie
ExecStart=/opt/swapie/swapie_backend
EnvironmentFile=/opt/swapie/.env
Restart=always
RestartSec=5
LimitNOFILE=65536

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/swapie/logs

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable swapie-backend
sudo systemctl start swapie-backend
```

### 4. Docker Deployment

```dockerfile
# Dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/swapie_backend /usr/local/bin/
COPY locales/ /opt/swapie/locales/
WORKDIR /opt/swapie
EXPOSE 8080
CMD ["swapie_backend"]
```

```bash
docker build -t swapie-backend .
docker run -d --env-file .env -p 8080:8080 swapie-backend
```

### 5. Nginx Reverse Proxy

```nginx
upstream swapie_backend {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 443 ssl http2;
    server_name api.swapie.app;

    ssl_certificate /etc/ssl/certs/swapie.pem;
    ssl_certificate_key /etc/ssl/private/swapie.key;

    client_max_body_size 20M;

    location / {
        proxy_pass http://swapie_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
    }
}
```

## Scaling Strategy

### Horizontal Scaling

```
                    ┌─────────────┐
                    │  Load       │
                    │  Balancer   │
                    │  (Nginx/    │
                    │  HAProxy)   │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
        ┌─────▼────┐ ┌────▼─────┐ ┌────▼─────┐
        │ Swapie   │ │ Swapie   │ │ Swapie   │
        │ Node 1   │ │ Node 2   │ │ Node 3   │
        │ (8 cores)│ │ (8 cores)│ │ (8 cores)│
        └─────┬────┘ └────┬─────┘ └────┬─────┘
              │            │            │
              └────────────┼────────────┘
                           │
                    ┌──────▼──────┐
                    │   MySQL     │
                    │ Primary +   │
                    │ Read        │
                    │ Replicas    │
                    └─────────────┘
```

### Scaling Steps

1. **Single Server (up to ~5K concurrent users)**
   - 1 Actix-web instance, `APP_WORKERS=<cpu_cores>`
   - MySQL on same server, `DATABASE_MAX_CONNECTIONS=50`

2. **Separated DB (up to ~20K concurrent users)**
   - Dedicated MySQL server (RDS/PlanetScale/self-hosted)
   - Increase connection pool: `DATABASE_MAX_CONNECTIONS=100`
   - Add Redis for session caching and rate limiting

3. **Multiple Backend Nodes (up to ~100K concurrent users)**
   - 2-4 backend instances behind load balancer
   - MySQL read replicas for read-heavy queries (book listings, search)
   - Shared Redis for rate limiting across nodes
   - S3/Spaces for file storage (already stateless)

4. **High Scale (100K+ concurrent users)**
   - Kubernetes deployment with HPA (Horizontal Pod Autoscaler)
   - MySQL cluster (Vitess/ProxySQL)
   - Dedicated Redis cluster
   - CDN for static assets and media
   - Consider splitting into microservices:
     - Auth service
     - Book/Matching service
     - Trade/Payment service
     - Chat service (WebSocket)

### Performance Characteristics

Actix-web is one of the fastest web frameworks:
- **Throughput**: ~200K+ requests/sec on a single core (JSON responses)
- **Latency**: sub-millisecond for simple endpoints
- **Memory**: ~10-30 MB per instance (vs ~100-300 MB for PHP/Laravel)
- **Startup**: <1 second (vs 5-15 seconds for Laravel)
- **Binary size**: ~10-15 MB (no runtime dependencies)

### Database Optimization

The MySQL schema includes indexes on:
- All foreign keys
- `books.user_id`, `books.status`, `books.type`
- `swipes(user_id, book_id)` unique
- `trades.initiator_id`, `trades.recipient_id`
- `messages.trade_id`, `messages.sender_id`
- Full-text search candidates: add `FULLTEXT(title, author, description)` on books table

### Recommended: Add Redis layer

```bash
# Add to .env
REDIS_URL=redis://127.0.0.1:6379

# Use for:
# - Rate limiting (shared across instances)
# - JWT blacklist (for logout/revoke)
# - Book listing cache (TTL: 60s)
# - User session cache
# - FCM token cache
```

## API Endpoints Summary

| Group | Count | Auth | Description |
|-------|-------|------|-------------|
| Auth | 15 | Mixed | Login, register, SMS, social, password reset |
| Books | 10 | Yes | CRUD, search, external ISBN, images |
| Swipe | 3 | Yes | Candidates, like/dislike/superlike |
| Offers | 8 | Yes | Create, accept, finalize, counter, cancel |
| Swap Center | 8 | Yes | Matches, likes, activity |
| Chat | 6 | Yes | Messages, inbox, read status |
| Profile | 11 | Yes | Update, address, GDPR export/delete |
| Delivery | 12 | Yes | InPost/Orlen lockers, tracking, disputes |
| Reviews | 3 | Yes | Create, status, user reviews |
| Payments | 12 | Yes | Wallet, Stripe, cards, top-up |
| Stripe Connect | 8 | Yes | Seller onboarding, bank accounts |
| Notifications | 7 | Yes | List, read, push tokens |
| Admin | 16 | Yes | Users, roles, permissions, settings, modules |
| Posts | 6 | Yes | CMS post types |
| Terms | 6 | Yes | Taxonomy terms |
| Public | 5 | No | Translations, genres, tags, regulations |
| **Total** | **136** | | |

## Security Features

- **JWT authentication** with configurable expiration
- **Argon2 password hashing** (memory-hard, resistant to GPU attacks)
- **Rate limiting** per endpoint group (login: 5/min, default: 60/min)
- **Input validation** on all request DTOs (validator crate)
- **SQL injection prevention** via parameterized queries (SQLx)
- **XSS prevention** via input sanitization
- **CORS configuration** for allowed origins
- **Stripe webhook signature verification**
- **GDPR compliance** (data export, account deletion, consent tracking)
- **Soft deletes** for data integrity
- **Idempotency keys** for messages and payments

## Adding a New Language

1. Create `locales/<lang_code>.json` with all translation keys
2. Update `src/i18n/mod.rs` to load the new locale file
3. Rebuild and deploy

## License

Proprietary — Swapie Team
