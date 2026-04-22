# Environment Migration

This release adds a generic image backend configuration for copy operations.

Old configuration:

```env
SKOPEO_PATH=/home/linuxbrew/.linuxbrew/bin/skopeo
```

New configuration:

```env
IMAGE_TOOL=skopeo
IMAGE_TOOL_PATH=/home/linuxbrew/.linuxbrew/bin/skopeo
IMAGE_TOOL_SRC_INSECURE=false
IMAGE_TOOL_DST_INSECURE=false
IMAGE_TOOL_EXTRA_INSPECT_ARGS=
IMAGE_TOOL_EXTRA_COPY_ARGS=
```

Backward compatibility:

- If `IMAGE_TOOL_PATH` is not set, SRM falls back to `SKOPEO_PATH`.
- Default `IMAGE_TOOL` is `skopeo`.

Recommended server migration:

1. Keep the old `SKOPEO_PATH` line during the first rollout.
2. Add the new `IMAGE_*` lines.
3. Restart the service.
4. Verify `/api/v1/version` shows the expected `image_tool`.
5. After verification, `SKOPEO_PATH` can stay as legacy fallback or be removed later.

Example for current `skopeo` deployment:

```env
IMAGE_TOOL=skopeo
IMAGE_TOOL_PATH=/home/linuxbrew/.linuxbrew/bin/skopeo
IMAGE_TOOL_SRC_INSECURE=false
IMAGE_TOOL_DST_INSECURE=false
IMAGE_TOOL_EXTRA_INSPECT_ARGS=
IMAGE_TOOL_EXTRA_COPY_ARGS=

SKOPEO_PATH=/home/linuxbrew/.linuxbrew/bin/skopeo
```

Example for `oci-patch` rollout:

```env
IMAGE_TOOL=oci-patch
IMAGE_TOOL_PATH=/home/mares/Development/Src/GoLang/oci-patch/oci-patch
IMAGE_TOOL_SRC_INSECURE=false
IMAGE_TOOL_DST_INSECURE=false
IMAGE_TOOL_EXTRA_INSPECT_ARGS=
IMAGE_TOOL_EXTRA_COPY_ARGS=

SKOPEO_PATH=/home/linuxbrew/.linuxbrew/bin/skopeo
```

TLS notes:

- `IMAGE_TOOL_SRC_INSECURE=true` affects source registry access.
- `IMAGE_TOOL_DST_INSECURE=true` affects target registry access.
- For `skopeo`, SRM maps these to `--src-tls-verify=false` and `--dest-tls-verify=false`.
- For `oci-patch`, SRM maps these to `--src-insecure` and `--dest-insecure`.
