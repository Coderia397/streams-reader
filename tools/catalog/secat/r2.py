"""Uploads the staged plan to R2 with the bucket-scoped S3 credentials in ~/.config/streams/nolabs-r2.env.

Keys of bodies and covers carry a content hash, so an object that already exists with the same size is skipped.
The index (catalog.json) is always written last, so the app never sees an index pointing at missing objects."""

from __future__ import annotations

from pathlib import Path

from .catalog import INDEX_KEY, Upload

ENV_FILE = Path.home() / ".config" / "streams" / "nolabs-r2.env"
REQUIRED = ("R2_ACCESS_KEY_ID", "R2_SECRET_ACCESS_KEY", "R2_ENDPOINT", "R2_BUCKET")


def read_env(path: Path = ENV_FILE) -> dict[str, str]:
    env: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        env[k.strip()] = v.strip().strip('"').strip("'")
    missing = [k for k in REQUIRED if not env.get(k)]
    if missing:
        raise RuntimeError(f"{path} lacks {', '.join(missing)}")
    return env


def client(env: dict[str, str]):
    import boto3
    from botocore.config import Config

    return boto3.client(
        "s3",
        endpoint_url=env["R2_ENDPOINT"],
        aws_access_key_id=env["R2_ACCESS_KEY_ID"],
        aws_secret_access_key=env["R2_SECRET_ACCESS_KEY"],
        region_name="auto",
        config=Config(retries={"max_attempts": 5, "mode": "standard"}, s3={"addressing_style": "path"}),
    )


def ordered(plan: list[Upload]) -> list[Upload]:
    return sorted(plan, key=lambda u: (u.key == INDEX_KEY, u.key))


def upload(plan: list[Upload], env: dict[str, str], log=print) -> dict[str, int]:
    s3 = client(env)
    bucket = env["R2_BUCKET"]
    stats = {"uploaded": 0, "skipped": 0, "bytes": 0}
    for u in ordered(plan):
        if u.key != INDEX_KEY and _exists(s3, bucket, u):
            stats["skipped"] += 1
            continue
        with u.path.open("rb") as f:
            s3.put_object(
                Bucket=bucket,
                Key=u.key,
                Body=f,
                ContentType=u.content_type,
                CacheControl=u.cache_control,
                Metadata={"sha256": u.sha256},
            )
        stats["uploaded"] += 1
        stats["bytes"] += u.size
        log(f"  put {u.key} ({u.size} B)")
    return stats


def _exists(s3, bucket: str, u: Upload) -> bool:
    from botocore.exceptions import ClientError

    try:
        head = s3.head_object(Bucket=bucket, Key=u.key)
    except ClientError as e:
        if e.response.get("Error", {}).get("Code") in ("404", "NoSuchKey", "NotFound"):
            return False
        raise
    return head.get("ContentLength") == u.size
