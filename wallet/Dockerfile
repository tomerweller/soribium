# Soribium wallet: static Vite build served by nginx, which reverse-proxies
# /api to the sequencer (same-origin → no CORS in the compose deployment).
# Portable across arch (pure JS build). Deploys unchanged to Vercel by
# pointing VITE_SEQUENCER_URL at the sequencer instead of using the proxy.
FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install
COPY . .
# The crypto vector gate runs in the build — a byte-mismatch fails the image.
RUN npm test -- --run
RUN npm run build

FROM nginx:alpine
COPY --from=build /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/templates/default.conf.template
# nginx:alpine's entrypoint runs envsubst over /etc/nginx/templates; default
# the sequencer URL for the compose network.
ENV SEQUENCER_URL=http://sequencer:8080
EXPOSE 80
