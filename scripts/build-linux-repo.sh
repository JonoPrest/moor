#!/usr/bin/env bash
# Fold newly built .deb/.rpm files into the APT and YUM repositories served
# from the gh-pages branch, and re-sign the indexes.
#
#   build-linux-repo.sh <pages-dir> <incoming-dir> <gpg-key-id>
#
# Layout under <pages-dir>:
#   nits-archive-keyring.gpg   public key, dearmored, for apt
#   nits.asc                   public key, armored, for rpm
#   deb/                       flat APT repo (Packages, Release, InRelease)
#   rpm/                       YUM repo (repodata/, signed repomd.xml)
#
# A flat repository keeps the client config to one line and means adding a new
# package never needs a new distribution or component.
set -euo pipefail

pages=${1:?pages directory}
incoming=${2:?incoming directory}
key_id=${3:?gpg key id}

deb_dir="$pages/deb"
rpm_dir="$pages/rpm"
mkdir -p "$deb_dir" "$rpm_dir"

# gpg needs the passphrase on stdin in batch mode; loopback lets us supply it.
gpg_sign() {
  gpg --batch --yes --pinentry-mode loopback \
      --passphrase "${GPG_PASSPHRASE:-}" --local-user "$key_id" "$@"
}

found=0
while IFS= read -r -d '' f; do
  cp "$f" "$deb_dir/"
  found=1
done < <(find "$incoming" -name '*.deb' -print0)
while IFS= read -r -d '' f; do
  cp "$f" "$rpm_dir/"
  found=1
done < <(find "$incoming" -name '*.rpm' -print0)

if [ "$found" -eq 0 ]; then
  echo "no .deb or .rpm in $incoming — nothing to do" >&2
  exit 0
fi

# --- APT -------------------------------------------------------------------
# Everything here runs *inside* $deb_dir. In a flat repository apt resolves each
# `Filename:` against the base URL, which already ends in /deb — so scanning
# from $pages would emit `deb/<pkg>.deb` and apt would fetch /deb/deb/<pkg>.deb
# and get a 404. Scanning `.` emits `./<pkg>.deb`, which resolves correctly.
( cd "$deb_dir"
  # Stale index files must not end up hashed into the Release they belong to.
  rm -f Packages Packages.gz Release Release.gpg InRelease

  dpkg-scanpackages --multiversion . > Packages
  gzip -9 -k -f Packages

  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin=nits \
    -o APT::FTPArchive::Release::Label=nits \
    -o APT::FTPArchive::Release::Suite=stable \
    -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Architectures="amd64 arm64" \
    -o APT::FTPArchive::Release::Components=main \
    release . > Release.tmp
  mv Release.tmp Release

  # Both signatures: detached for older apt, inline for `signed-by` clients.
  gpg_sign --armor --detach-sign --output Release.gpg Release
  gpg_sign --clearsign --output InRelease Release
)

# --- YUM -------------------------------------------------------------------
createrepo_c --update "$rpm_dir"
gpg_sign --armor --detach-sign --output "$rpm_dir/repodata/repomd.xml.asc" \
         "$rpm_dir/repodata/repomd.xml"

# --- public key + landing page ---------------------------------------------
gpg --export --armor "$key_id" > "$pages/nits.asc"
gpg --export "$key_id" > "$pages/nits-archive-keyring.gpg"

cat > "$pages/index.html" <<'HTML'
<!doctype html>
<meta charset="utf-8">
<title>Nits package repositories</title>
<style>body{font:15px/1.6 system-ui,sans-serif;max-width:44rem;margin:3rem auto;padding:0 1rem}pre{background:#f4f4f5;padding:.9rem;border-radius:6px;overflow-x:auto}</style>
<h1>Nits package repositories</h1>
<h2>Debian / Ubuntu</h2>
<pre>curl -fsSL https://jonoprest.github.io/nits/nits-archive-keyring.gpg \
  | sudo tee /usr/share/keyrings/nits-archive-keyring.gpg > /dev/null
echo "deb [signed-by=/usr/share/keyrings/nits-archive-keyring.gpg] https://jonoprest.github.io/nits/deb ./" \
  | sudo tee /etc/apt/sources.list.d/nits.list
sudo apt update &amp;&amp; sudo apt install nits</pre>
<h2>Fedora / RHEL</h2>
<pre>sudo rpm --import https://jonoprest.github.io/nits/nits.asc
sudo tee /etc/yum.repos.d/nits.repo &lt;&lt;'EOF'
[nits]
name=Nits
baseurl=https://jonoprest.github.io/nits/rpm
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://jonoprest.github.io/nits/nits.asc
EOF
sudo dnf install nits</pre>
HTML

# gh-pages would otherwise hide directories whose names Jekyll dislikes.
touch "$pages/.nojekyll"
