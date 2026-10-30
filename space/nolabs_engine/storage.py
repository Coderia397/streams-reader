"""R2 access over its S3 API. Audio goes straight from the Space to R2, so the Worker never carries audio bytes.

The credential is scoped to Object Read + Write on `nolabs-media` only (decision D7). Keys are validated by contracts.py
before they reach this module.
"""
from __future__ import annotations

import hashlib
import os
import tempfile
import threading

import boto3
from botocore.config import Config

AUDIO_CACHE_CONTROL = "public, max-age=31536000, immutable"
# A raw recording is at most a few minutes of WAV; refuse anything far larger before reading it into memory.
MAX_DOWNLOAD_BYTES = 64 * 1024 * 1024
ENV_VARS = ("R2_ENDPOINT", "R2_ACCESS_KEY_ID", "R2_SECRET_ACCESS_KEY", "R2_BUCKET")


class Storage:
    def __init__(self, endpoint: str, access_key: str, secret_key: str, bucket: str):
        self.bucket = bucket

        def client(attempts: int, timeout: int):
            return boto3.client(
                "s3", endpoint_url=endpoint, aws_access_key_id=access_key, aws_secret_access_key=secret_key,
                region_name="auto",
                config=Config(retries={"max_attempts": attempts, "mode": "standard"}, connect_timeout=timeout,
                              read_timeout=60 if attempts > 1 else timeout),
            )

        self._s3 = client(4, 10)
        # Health must answer quickly even when R2 is down: one attempt, short timeouts.
        self._probe = client(1, 5)
        self._ref_dir = tempfile.mkdtemp(prefix="refs-")
        self._lock = threading.Lock()

    def put_audio(self, key: str, data: bytes) -> None:
        self._s3.put_object(Bucket=self.bucket, Key=key, Body=data, ContentType="audio/webm",
                            CacheControl=AUDIO_CACHE_CONTROL)

    def put_json(self, key: str, text: str) -> None:
        self._s3.put_object(Bucket=self.bucket, Key=key, Body=text.encode("utf-8"),
                            ContentType="application/json; charset=utf-8")

    def put_wav(self, key: str, data: bytes) -> None:
        self._s3.put_object(Bucket=self.bucket, Key=key, Body=data, ContentType="audio/wav")

    def get_bytes(self, key: str) -> bytes:
        obj = self._s3.get_object(Bucket=self.bucket, Key=key)
        size = int(obj.get("ContentLength") or 0)
        if size > MAX_DOWNLOAD_BYTES:
            obj["Body"].close()
            raise ValueError(f"{key} is {size} bytes, limit {MAX_DOWNLOAD_BYTES}")
        return obj["Body"].read()

    def download(self, key: str) -> str:
        """Fetch a reference clip to a local file named by its content hash and return the path. Ming caches prompt
        features by path, so a re-recorded voice (same key, new bytes) must get a new path."""
        data = self.get_bytes(key)
        path = os.path.join(self._ref_dir, f"{hashlib.sha256(data).hexdigest()}{os.path.splitext(key)[1]}")
        with self._lock:
            if not os.path.exists(path):
                tmp = f"{path}.part"
                with open(tmp, "wb") as f:
                    f.write(data)
                os.replace(tmp, path)
        return path

    def ping(self) -> bool:
        """True when the bucket answers HeadBucket with these credentials."""
        try:
            self._probe.head_bucket(Bucket=self.bucket)
            return True
        except Exception as e:  # noqa: BLE001 - any failure means storage is not usable
            print(f"storage check failed: {type(e).__name__}: {e}", flush=True)
            return False


def from_env() -> Storage:
    missing = [k for k in ENV_VARS if not os.environ.get(k)]
    if missing:
        raise RuntimeError(f"missing Space secrets: {', '.join(missing)}")
    return Storage(*(os.environ[k] for k in ENV_VARS))
