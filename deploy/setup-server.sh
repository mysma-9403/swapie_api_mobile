#!/bin/bash
# ── Swapie Backend - Server Setup Script ──────────────────────────────────────
# Run this ONCE on a fresh server to set up everything.
# Usage: ssh root@your-server 'bash -s' < deploy/setup-server.sh
set -euo pipefail

echo "=== Swapie Backend Server Setup ==="

# ── 1. System packages ───────────────────────────────────────────────────────
apt-get update && apt-get upgrade -y
apt-get install -y \
    docker.io docker-compose-plugin \
    nginx certbot python3-certbot-nginx \
    ufw fail2ban curl

# ── 2. Create swapie user ────────────────────────────────────────────────────
useradd -r -m -s /bin/bash swapie || true
usermod -aG docker swapie

# ── 3. Firewall ──────────────────────────────────────────────────────────────
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable

# ── 4. Create app directory ──────────────────────────────────────────────────
mkdir -p /opt/swapie
chown swapie:swapie /opt/swapie

# ── 5. SSL Certificate ──────────────────────────────────────────────────────
# First, point api.swapie.pl DNS to this server, then:
certbot certonly --nginx -d api.swapie.pl --non-interactive --agree-tos -m admin@swapie.pl || \
    echo "SSL cert failed - make sure DNS is pointed to this server first"

# Auto-renew cron
echo "0 3 * * * certbot renew --quiet --post-hook 'systemctl reload nginx'" | crontab -

# ── 6. Nginx config ──────────────────────────────────────────────────────────
cp /opt/swapie/deploy/proxy_params /etc/nginx/proxy_params
cp /opt/swapie/deploy/nginx.conf /etc/nginx/sites-available/api.swapie.pl
ln -sf /etc/nginx/sites-available/api.swapie.pl /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default
nginx -t && systemctl restart nginx

# ── 7. Docker login to GHCR ─────────────────────────────────────────────────
echo "=== Setup complete! ==="
echo ""
echo "Next steps:"
echo "1. Copy .env to /opt/swapie/.env and fill in production values"
echo "2. Copy docker-compose.yml to /opt/swapie/"
echo "3. Copy deploy/ directory to /opt/swapie/deploy/"
echo "4. Login to GHCR: echo 'TOKEN' | docker login ghcr.io -u USERNAME --password-stdin"
echo "5. Start services: cd /opt/swapie && docker compose up -d"
echo "6. Set up GitHub Actions secrets (see README)"
