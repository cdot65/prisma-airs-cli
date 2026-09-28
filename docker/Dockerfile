FROM node:20-alpine AS build

RUN corepack enable && corepack prepare pnpm@10.6.5 --activate

WORKDIR /app

COPY package.json pnpm-lock.yaml LICENSE NOTICE ./
RUN pnpm install --frozen-lockfile

COPY tsconfig.json tsup.config.ts ./
COPY src/ src/
RUN pnpm run build

FROM node:20-alpine
LABEL org.opencontainers.image.source=https://git.cdot.io/cdot/prisma-airs-cli
LABEL org.opencontainers.image.licenses=Apache-2.0

RUN apk add --no-cache fontconfig ttf-dejavu

RUN corepack enable && corepack prepare pnpm@10.6.5 --activate

WORKDIR /app

COPY package.json pnpm-lock.yaml LICENSE NOTICE ./
RUN pnpm install --frozen-lockfile --prod

COPY --from=build /app/dist/ dist/

VOLUME /root/.prisma-airs

ENTRYPOINT ["node", "dist/cli/index.js"]
