# Entorno para compilar el binario de Linux en la distribución más antigua que admitimos
# (Ubuntu 24.04: glibc 2.39, libheif 1.17). Un binario compilado aquí funciona también en las
# distribuciones más nuevas; uno compilado en Arch solo funciona en las que tengan su misma glibc.
# Lo usa `packaging/dist.sh linux`.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl pkg-config file \
      libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libheif-dev \
 && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
