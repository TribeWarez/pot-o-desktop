FROM rust:1.85-slim-bookworm AS build

# Tauri system deps
RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    javascriptcoregtk-4.1 \
    libsoup-3.0 \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    pkg-config \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Node.js for frontend build
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get install -y nodejs && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for dependency caching
COPY package.json package-lock.json ./
RUN npm ci

COPY src-tauri/Cargo.toml src-tauri/Cargo.toml
COPY src-tauri/Cargo.lock src-tauri/Cargo.lock
RUN mkdir -p src-tauri/src && echo "fn main() {}" > src-tauri/src/main.rs
RUN cargo build --manifest-path src-tauri/Cargo.toml --release 2>/dev/null || true

# Copy project files
COPY . .

# Build frontend
RUN npm run build

# Build Tauri binary
RUN cargo build --manifest-path src-tauri/Cargo.toml --release

# ── Runtime image ──────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-0 \
    libayatana-appindicator3-dev \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /app/src-tauri/target/release/pot-o-desktop /usr/local/bin/pot-o-desktop

ENTRYPOINT ["pot-o-desktop"]
