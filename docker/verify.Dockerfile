# Verify exact published bytes with network disabled during execution.
ARG IMAGE
FROM ${IMAGE}
COPY .github/scripts/ /verification/
RUN --network=none node /verification/verify-dlp-consumer.mjs /app/dist/cli/index.js
