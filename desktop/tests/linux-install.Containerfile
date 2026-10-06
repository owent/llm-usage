FROM docker.io/library/debian@sha256:7792b1f7702a86946cd518db72b6a407302c3e9bc1635634368b878189e8221c
ARG WITH_SCREEN_READER=0
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    libgtk-3-0 libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-2 \
    webkit2gtk-driver xvfb xauth dbus-x11 gnome-keyring openbox \
    python3 fonts-dejavu-core fonts-noto-cjk ca-certificates curl x11-utils \
    libfuse2t64 fuse3 procps sudo \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 acceptance
WORKDIR /workspace/llm-usage
RUN if test "$WITH_SCREEN_READER" = 1; then apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        orca speech-dispatcher speech-dispatcher-espeak-ng python3-pyatspi xdotool \
    && rm -rf /var/lib/apt/lists/*; fi
RUN mkdir -p build/install-lifecycle && chown -R acceptance:acceptance /workspace/llm-usage
