# Toolchain image for Jenkins: bake pnpm in once. It can't be fetched with npx at
# build time because npm rejects package.json's devEngines (pnpm, not npm).
FROM node:26
RUN npm install -g pnpm@12.15.0
