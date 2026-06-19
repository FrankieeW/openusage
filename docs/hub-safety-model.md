# Hub Safety Model

Hub installs plugins from source repositories and local folders. This page
summarizes what Hub checks before installing and what users should look at in
the install preview.

## Source Trust

Hub labels sources by trust level:

- **default**: the built-in recommended source.
- **Community**: a user-added GitHub source.
- **Local Development**: a local folder on this machine.
- **Unknown Git Source**: a non-GitHub git source.

Unknown Git Source installs require an explicit preview confirmation.

## Install Safety

Hub does not run install scripts. A plugin is copied as files from
`plugins/<id>/`.

Before replacing an installed plugin, Hub copies the new plugin to a temporary
directory, checks `plugin.json`, checks the `entry` file, calculates the package
hash, writes install metadata into the temporary copy, then renames it into
place. The previous installed copy is moved into Hub's local trash folder before
the new copy replaces it.

Uninstall also moves the plugin into Hub's local trash folder instead of
deleting it directly.

## Package Hashes

The package hash is calculated from the files under `plugins/<id>/`.

Local install metadata, `.openusage-install.json`, is not part of the hash. That
file records where the installed copy came from, which version was installed,
the source branch, and the source commit when available.

The same plugin id does not always mean the same plugin. Two sources can publish
the same id and version with different files. Hub compares package hashes to
show whether a source provides the same package or a different package with the
same id.

## Source Refresh

For git sources, Hub preserves the selected branch. Refresh results show the
branch, commit sha, refresh time, discovered plugin count, skipped plugin count,
and skipped reasons.

Plugin cards can show an update date when the plugin manifest includes
`updatedAt`. This is publisher-declared plugin metadata, not the time the user
installed or refreshed the source.

Hub stores `hub-cache-index.json` in the source cache. If the source commit has
not changed, Hub can reuse the cached plugin summaries and package hashes
instead of recalculating them.
