#!/bin/sh
# shellcheck shell=dash

set -eu

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Usage: scripts/create-release.sh TAG [options]

Creates a GitHub Release for TAG, targeting main by default. The release event
triggers .github/workflows/release-assets.yml, which uploads release archives.

Options:
  --target REF       Commit, branch, or tag to release from (default: main)
  --title TITLE     Release title (default: "JEdit TAG")
  --notes NOTES     Release notes text
  --notes-file PATH Read release notes from a file
  --final           Publish as a full release instead of a prerelease
  --no-watch        Do not wait for the release asset workflow
  -h, --help        Show this help

Examples:
  scripts/create-release.sh v20.0.0-rc.1
  scripts/create-release.sh v20.0.0 --final --notes-file RELEASE_NOTES.md
EOF
}

[ "$#" -gt 0 ] || {
    usage
    exit 1
}

case "$1" in
    -h|--help)
        usage
        exit 0
        ;;
esac

tag=$1
shift

target=main
title=
notes=
notes_file=
prerelease=1
watch=1
workflow=release-assets.yml

while [ "$#" -gt 0 ]; do
    case "$1" in
        --target)
            [ "$#" -ge 2 ] || die "--target requires a value"
            target=$2
            shift 2
            ;;
        --title)
            [ "$#" -ge 2 ] || die "--title requires a value"
            title=$2
            shift 2
            ;;
        --notes)
            [ "$#" -ge 2 ] || die "--notes requires a value"
            notes=$2
            shift 2
            ;;
        --notes-file)
            [ "$#" -ge 2 ] || die "--notes-file requires a value"
            notes_file=$2
            shift 2
            ;;
        --final)
            prerelease=0
            shift
            ;;
        --no-watch)
            watch=0
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown option: $1"
            ;;
    esac
done

case "$tag" in
    v[0-9]*.[0-9]*.[0-9]*)
        ;;
    *)
        die "tag must look like v20.0.0 or v20.0.0-rc.1"
        ;;
esac

[ -z "$title" ] && title="JEdit $tag"
[ -n "$notes" ] || notes="JEdit release $tag."

command -v gh >/dev/null 2>&1 || die "gh is required. Install GitHub CLI and run gh auth login."
gh auth status >/dev/null 2>&1 || die "gh is not authenticated. Run gh auth login."

if gh release view "$tag" >/dev/null 2>&1; then
    die "release $tag already exists"
fi

prerelease_args=
if [ "$prerelease" = 1 ]; then
    prerelease_args=--prerelease
fi

if [ "$prerelease" = 1 ]; then
    release_kind=prerelease
else
    release_kind=release
fi

printf 'Creating %s %s from %s...\n' "$release_kind" "$tag" "$target"

if [ -n "$notes_file" ]; then
    [ -f "$notes_file" ] || die "notes file not found: $notes_file"
    gh release create "$tag" \
        --target "$target" \
        --title "$title" \
        $prerelease_args \
        --notes-file "$notes_file"
else
    gh release create "$tag" \
        --target "$target" \
        --title "$title" \
        $prerelease_args \
        --notes "$notes"
fi

[ "$watch" = 1 ] || exit 0

printf 'Waiting for %s to start for %s...\n' "$workflow" "$tag"
run_id=
tries=0
while [ -z "$run_id" ] && [ "$tries" -lt 40 ]; do
    run_id=$(gh run list \
        --workflow "$workflow" \
        --limit 20 \
        --json databaseId,event,headBranch \
        --jq ".[] | select(.event == \"release\" and .headBranch == \"$tag\") | .databaseId" \
        | head -n 1)
    if [ -z "$run_id" ]; then
        tries=$((tries + 1))
        sleep 3
    fi
done

[ -n "$run_id" ] || die "timed out waiting for $workflow to start"

gh run watch "$run_id" --exit-status

printf '\nRelease assets:\n'
gh release view "$tag" --json assets --jq '.assets[].name'
