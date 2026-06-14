FROM node:20-bookworm-slim

WORKDIR /app
COPY tests/mock-https/package.json ./
RUN npm install --omit=dev
COPY tests/mock-https/server.js ./

EXPOSE 8443
CMD ["node", "server.js"]
