"""R2 access over its S3 API. Audio goes straight from the Space to R2 so the Worker never carries audio bytes."""
from __future__ import annotations

import os
import tempfile
from functools import lru_cache

import boto3
from botocore.config import Config


class Storage:
    def __init__(self, endpoint: str, access_key: str, secret_key: str, bucket: str):
        self.bucket = bucket
        self._s3 = boto3.client(
            "s3",
            endpoint_url=endpoint,
            aws_access_key_id=access_key,
            aws_secret_access_key=secret_key,
            region_name="auto",
            config=Config(retries={"max_attempts": 4, "mode": "standard"}, connect_timeout=10, read_timeout=60),
        )

    def put_audio(self, key: str, data: bytes) -> None:
        self._s3.put_object(
            Bucket=self.bucket, Key=key, Body=data, ContentType="audio/ogg",
            CacheControl="public, max-age=31536000, immutable",
        )

    def put_wav(self, key: str, data: bytes) -> None:
        self._s3.put_object(Bucket=self.bucket, Key=key, Body=data, ContentType="audio/wav")

    def exists(self, key: str) -> bool:
        try:
            self._s3.head_object(Bucket=self.bucket, Key=key)
            return True
        except self._s3.exceptions.ClientError:
            return False

    def download(self, key: str) -> str:
        """Fetch an object to a local file once per process and return its path."""
        return _cached_download(self, key)


@lru_cache(maxsize=64)
def _cached_download(storage: Storage, key: str) -> str:
    suffix = os.path.splitext(key)[1] or ".bin"
    handle, path = tempfile.mkstemp(prefix="ref-", suffix=suffix)
    os.close(handle)
    storage._s3.download_file(storage.bucket, key, path)
    return path


def from_env() -> Storage:
    missing = [k for k in ("R2_ENDPOINT", "R2_ACCESS_KEY_ID", "R2_SECRET_ACCESS_KEY", "R2_BUCKET") if not os.environ.get(k)]
    if missing:
        raise RuntimeError(f"missing Space secrets: {', '.join(missing)}")
    return Storage(os.environ["R2_ENDPOINT"], os.environ["R2_ACCESS_KEY_ID"], os.environ["R2_SECRET_ACCESS_KEY"],
                   os.environ["R2_BUCKET"])
