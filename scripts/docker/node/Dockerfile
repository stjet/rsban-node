FROM ubuntu:22.04 as builder

ARG COMPILER=gcc
ARG NANO_NETWORK=live

# Install build dependencies
COPY ./scripts/ci/prepare/linux /tmp/prepare
RUN /tmp/prepare/prepare.sh

COPY ./ /tmp/src
WORKDIR /tmp/src

#Define ARGs for ci/build-node.sh
ARG BUILD_TYPE=RelWithDebInfo
ARG NANO_TEST=OFF
ARG NANO_TRACING=OFF
ARG COVERAGE=OFF
ARG CMAKE_SANITIZER=""
ARG CI_TAG=DEV_BUILD
ARG CI_VERSION_PRE_RELEASE=OFF
ARG SANITIZER

# Build node
ENV PATH="/root/.cargo/bin:${PATH}"
RUN cd main && cargo build --release
RUN echo ${NANO_NETWORK} >/etc/banano-network

FROM ubuntu:22.04

RUN groupadd --gid 1000 bananode && \
    useradd --uid 1000 --gid bananode --shell /bin/bash --create-home bananode

COPY --from=builder /tmp/src/target/release/rsban_node /usr/bin
COPY --from=builder /etc/banano-network /etc

COPY scripts/docker/node/config /usr/share/banano/config
RUN ldconfig

WORKDIR /root
USER root

ENV PATH="${PATH}:/usr/bin"
ENTRYPOINT ["/usr/bin/rsban_node"]
CMD ["node", "run"]

ARG REPOSITORY=banano/rsbanano-node
LABEL org.opencontainers.image.source https://github.com/$REPOSITORY
