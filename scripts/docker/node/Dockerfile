FROM alpine:3.22 AS builder

ARG COMPILER=gcc
ARG NANO_NETWORK=live

RUN apk add --no-cache \
    bash \
    musl-dev \
    cmake \
    clang \
    llvm \
    linux-headers

COPY ./scripts/ci/prepare/linux /tmp/prepare
RUN bash /tmp/prepare/prepare.sh

COPY ./ /tmp/src
WORKDIR /tmp/src

ARG BUILD_TYPE=RelWithDebInfo
ARG NANO_TEST=OFF
ARG NANO_TRACING=OFF
ARG COVERAGE=OFF
ARG CMAKE_SANITIZER=""
ARG CI_TAG=DEV_BUILD
ARG CI_VERSION_PRE_RELEASE=OFF
ARG SANITIZER

ENV PATH="/root/.cargo/bin:${PATH}"
RUN apk add --no-cache bash musl-dev \
    && apk add --no-cache --virtual .build-deps cmake clang llvm linux-headers \
    && cd main && cargo build --release \
    && strip /tmp/src/target/release/rsnano_node \
    && apk del .build-deps

RUN echo ${NANO_NETWORK} >/etc/nano-network


FROM alpine:3.22

RUN addgroup -g 1000 nanocurrency && \
    adduser -u 1000 -G nanocurrency -s /bin/sh -D nanocurrency

COPY --from=builder /tmp/src/target/release/rsnano_node /usr/bin
COPY --from=builder /etc/nano-network /etc
COPY scripts/docker/node/config /usr/share/nano/config

WORKDIR /root
USER root

ENV PATH="${PATH}:/usr/bin"
ENTRYPOINT ["/usr/bin/rsnano_node"]
CMD ["node", "run"]

ARG REPOSITORY=banano/rsban
LABEL org.opencontainers.image.source="https://github.com/$REPOSITORY"
