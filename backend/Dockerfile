# Stage 1: Build dependencies
FROM python:3.12-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    libhdf5-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies to a virtual env
RUN python -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Stage 2: Runtime
FROM python:3.12-slim AS runtime

WORKDIR /app

# Install runtime dependencies only (libhdf5 for h5py)
RUN apt-get update && apt-get install -y --no-install-recommends \
    libhdf5-serial-dev \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Copy virtual env from builder
COPY --from=builder /opt/venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Copy application code
COPY radar_server.py .

# Copy static files (optional - create dir if not exists)
RUN mkdir -p /app/static
COPY static/ ./static/

# Create non-root user for security
RUN useradd --create-home --shell /bin/bash appuser
USER appuser

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import httpx; httpx.get('http://localhost:8080/api/health', timeout=5).raise_for_status()"

# Run with uvicorn
CMD ["uvicorn", "radar_server:app", "--host", "0.0.0.0", "--port", "8080"]
