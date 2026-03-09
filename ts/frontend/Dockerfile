# =============================================================================
# Frontend Dockerfile
# =============================================================================
#
# Two-stage build:
#   Stage 1 (builder) – Node/Bun installs npm deps and runs `vite build`,
#                        producing a /dist directory of static HTML/JS/CSS.
#   Stage 2 (runtime) – nginx serves those static files on port 80.
#
# The browser then connects DIRECTLY to the moq-relay container on port 4443
# for the real-time MoQ pub/sub; no network traffic goes through nginx
# for the QUIC/WebSocket streams.
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build the Vite/TypeScript frontend
# -----------------------------------------------------------------------------
FROM node:22-alpine AS builder

WORKDIR /app

# Copy dependency manifests first to leverage the image layer cache.
# If package.json doesn't change, npm ci won't redownload packages on
# subsequent builds -- even if source files change.
COPY package.json package-lock.json* ./

# Install all dependencies (including devDeps needed for vite build).
RUN npm install

# Now copy the rest of the source files.
COPY tsconfig.json vite.config.ts index.html ./
COPY src/ ./src/

# VITE_RELAY_URL is baked into the JS bundle at build time.
# Default: http://localhost:4443 (relay exposed on the host).
# Override at build time:  podman build --build-arg VITE_RELAY_URL=http://myrelay:4443 .
ARG VITE_RELAY_URL=http://localhost:4443
ENV VITE_RELAY_URL=$VITE_RELAY_URL

# Run the Vite production build.
# Output goes to /app/dist (static HTML + hashed JS/CSS bundles).
RUN npm run build

# -----------------------------------------------------------------------------
# Stage 2: Serve with nginx
# -----------------------------------------------------------------------------
FROM nginx:1.27-alpine AS runtime

# Remove the default nginx configuration.
RUN rm /etc/nginx/conf.d/default.conf

# Install our custom nginx.conf.
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Copy the built assets from the builder stage.
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
