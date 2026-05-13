# renovate: datasource=github-release-attachments depName=rust-lang/rustup
ARG RUSTUP_VERSION=1.29.0
# renovate: datasource=github-release-attachments depName=rust-lang/rustup digestVersion=1.29.0
ARG RUSTUP_SUM_arm64=88761caacddb92cd79b0b1f939f3990ba1997d701a38b3e8dd6746a562f2a759
# renovate: datasource=github-release-attachments depName=rust-lang/rustup digestVersion=1.29.0
ARG RUSTUP_SUM_amd64=9cd3fda5fd293890e36ab271af6a786ee22084b5f6c2b83fd8323cec6f0992c1

FROM --platform=${BUILDPLATFORM} ghcr.io/cross-rs/aarch64-unknown-linux-musl:0.2.5 AS build-arm64
ARG BUILDPLATFORM
ARG TARGETPLATFORM
ARG RUSTUP_VERSION
ARG RUSTUP_SUM_arm64

RUN curl --proto '=https' --tlsv1.2 -sSfL -o /tmp/rustup-init \
    "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/aarch64-unknown-linux-musl/rustup-init" && \
    echo "${RUSTUP_SUM_arm64}  /tmp/rustup-init" | sha256sum -c - && \
    chmod +x /tmp/rustup-init && \
    /tmp/rustup-init -y --target aarch64-unknown-linux-musl --default-toolchain stable && \
    rm /tmp/rustup-init

ENV PATH=/root/.cargo/bin:$PATH
RUN cargo --version

WORKDIR /usr/src

RUN mkdir /usr/src/controller
WORKDIR /usr/src/controller
COPY ./ ./

ARG features=""
RUN cargo install --locked --target aarch64-unknown-linux-musl --features=${features} --path .

FROM --platform=${BUILDPLATFORM} ghcr.io/cross-rs/x86_64-unknown-linux-musl:0.2.5 AS build-amd64
ARG BUILDPLATFORM
ARG TARGETPLATFORM
ARG RUSTUP_VERSION
ARG RUSTUP_SUM_amd64

RUN curl --proto '=https' --tlsv1.2 -sSfL -o /tmp/rustup-init \
    "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-musl/rustup-init" && \
    echo "${RUSTUP_SUM_amd64}  /tmp/rustup-init" | sha256sum -c - && \
    chmod +x /tmp/rustup-init && \
    /tmp/rustup-init -y --target x86_64-unknown-linux-musl --default-toolchain stable && \
    rm /tmp/rustup-init

ENV PATH=/root/.cargo/bin:$PATH
RUN cargo --version

WORKDIR /usr/src

RUN mkdir /usr/src/controller
WORKDIR /usr/src/controller
COPY ./ ./

ARG features=""
RUN cargo install --locked --target x86_64-unknown-linux-musl --features=${features} --path .

FROM --platform=amd64 registry.suse.com/suse/helm:3.17 AS helm-amd64
FROM --platform=arm64 registry.suse.com/suse/helm:3.17 AS helm-arm64

FROM helm-amd64 AS target-amd64
COPY --from=build-amd64 --chmod=0755 /root/.cargo/bin/controller /apps/controller

FROM helm-arm64 AS target-arm64
COPY --from=build-arm64 --chmod=0755 /root/.cargo/bin/controller /apps/controller

FROM target-${TARGETARCH}
ENV PATH="${PATH}:/apps"
EXPOSE 8080
ENTRYPOINT ["/apps/controller"]
