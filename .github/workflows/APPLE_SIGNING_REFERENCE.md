# Apple Certificate Signing Reference
#
# This file documents how to enable Apple code signing in the nightly workflow.
# Uncomment the relevant sections in .github/workflows/nightly.yml when you have
# the required certificates and secrets configured.
#
# =============================================================================
# STEP 1: Get Apple Developer Certificate
# =============================================================================
#
# Option A: From existing Mac with certificate installed
#   1. Open Keychain Access
#   2. Find your Apple Development or Distribution certificate
#   3. Right-click → Export → Save as .p12 (set a password)
#   4. Convert to base64:
#      base64 -i certificate.p12 -o cert.txt
#      cat cert.txt  # Copy this as APPLE_CERTIFICATE secret
#
# Option B: Create new certificate
#   1. Go to https://developer.apple.com/account
#   2. Certificates → + → Apple Distribution
#   3. Generate CSR using Keychain Access (Keychain Access → Certificate Assistant → Request Certificate)
#   4. Download and install the certificate
#   5. Export as .p12 (see above)
#
# =============================================================================
# STEP 2: Add GitHub Secrets
# =============================================================================
#
# Go to: https://github.com/FrankieeW/openusage/settings/secrets/actions
#
# Add these secrets:
#
#   APPLE_CERTIFICATE
#     - Value: The base64-encoded .p12 certificate (without newlines)
#     - Get it: base64 -w0 certificate.p12
#
#   APPLE_CERTIFICATE_PASSWORD
#     - Value: The password you set when exporting the .p12
#
#   KEYCHAIN_PASSWORD
#     - Value: A random password for the build keychain (e.g., generate with `openssl rand -base64 32`)
#
#   APPLE_SIGNING_IDENTITY
#     - Value: Your certificate name (e.g., "Apple Distribution: Your Name (TEAMID)")
#
#   APPLE_ID
#     - Value: Your Apple Developer email
#
#   APPLE_PASSWORD
#     - Value: Your Apple Developer app-specific password
#     - Get it: https://appleid.apple.com/account/manage → App-Specific Passwords
#
#   APPLE_TEAM_ID
#     - Value: Your Apple Team ID (from developer.apple.com membership details)
#
# =============================================================================
# STEP 3: Enable in Workflow
# =============================================================================
#
# In .github/workflows/nightly.yml, uncomment these sections:
#
# 1. Apple certificate import step:
#
#   - name: Import Apple Developer Certificate
#     env:
#       APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
#       APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
#       KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
#     run: |
#       echo "$APPLE_CERTIFICATE" | base64 --decode > certificate.p12
#       security create-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
#       security default-keychain -s build.keychain
#       security unlock-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
#       security set-keychain-settings -t 3600 -u build.keychain
#       security import certificate.p12 -k build.keychain -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
#       security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" build.keychain
#       rm certificate.p12
#
# 2. Add these env vars to the Build step:
#
#   APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
#   APPLE_ID: ${{ secrets.APPLE_ID }}
#   APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
#   APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
