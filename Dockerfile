FROM python:3.13-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      build-essential \
      curl \
      ca-certificates \
      gnupg \
    && mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
      | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg \
    && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_20.x nodistro main" \
      > /etc/apt/sources.list.d/nodesource.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY package.json package-lock.json* ./
RUN npm install --omit=dev

COPY . .

ENV PYTHONPATH=/app
ENV DATABASE_PATH=/data/monitoring.db
ENV HOST=0.0.0.0
ENV PORT=8000

EXPOSE 8000

CMD ["python", "-m", "web.main"]
