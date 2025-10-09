# build stage
FROM node:lts-alpine AS build-stage
WORKDIR /app
COPY package*.json /app/
RUN npm install
COPY . /app
RUN npm run build

# production stage
FROM nginx:stable-alpine AS production-stage
COPY --from=build-stage /app/dist /usr/share/nginx/html
COPY contrib/entrypoint.sh /
RUN chmod +x /entrypoint.sh
COPY contrib/nginx.conf etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["/entrypoint.sh"]