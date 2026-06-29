# Build the React UI to static assets, then serve them with Caddy, which also
# reverse-proxies /api to the API container and terminates HTTPS (Let's Encrypt).

FROM node:20-slim AS builder
WORKDIR /ui
COPY zavora-erp-ui/package.json zavora-erp-ui/package-lock.json* ./
RUN npm install
COPY zavora-erp-ui ./
RUN npm run build

FROM caddy:2-alpine AS runtime
COPY --from=builder /ui/dist /srv
COPY deploy/Caddyfile /etc/caddy/Caddyfile
EXPOSE 80 443
