# Release Checklist

These steps assume that you've checked out the Soketto repository and are in the root directory of it.

1. Ensure that everything you'd like to see released is on the `develop` branch.

2. Create a release branch off `develop`, for example `release-v0.6.0`. The branch name should start with `release`
   in order that we can target commits with CI. You'll want to decide how far the version needs to be bumped 
   based on the changes to date. If unsure what to bump the version to (eg is it a major, minor or patch 
   release), check with the Parity Tools team.

2. Make sure that the tests pass.
   
   ```
   cargo test
   ```

   If anything fails, it's likely worth fixing the failures in a PR and postponing the release. Minor 
   issues can be fixed on this release branch.

3. Check that you're happy with the current documentation.
   
   ```
   cargo doc --open
   ```
 
   It's probably a good idea to check for broken links in the documentation, which
   can be done using the third party tool `cargo-deadlinks`:

   ```
   cargo install cargo-deadlinks
   cargo deadlinks --check-http
   ```

   If there are minor issues with the documentation, they can be fixed in the release branch.

4. Bump the crate version in `Cargo.toml` to whatever was decided in step 2.

5. Update `CHANGELOG.md` to reflect the difference between this release and last. If you're unsure of
   what to add, check with the Tools team. 
   
   One way to gain some inspiration about what to write is by looking through the commit history since 
   the last version (eg `git log --pretty LAST_VERSION_TAG..HEAD`).

   Alternately, look at the commit history: https://github.com/paritytech/soketto/commits/develop.

6. Commit any of the above changes to the release branch and open a PR in GitHub with a base of `develop`.

7. Once the branch has been reviewed, we are ready to publish this branch to crates.io.
   
   First, do a dry run to make sure that things seem sane:
   ```
   cargo publish --dry-run
   ```

   If we're happy with everything, proceed with the release:
   ```
   cargo publish
   ```

8. If the release was successful, then tag the latest commit in our release branch so that somebody can
   get back to this exact state in the future.

   ```
   git tag v0.6.0 # use the version number you've just published to crates.io
   git push --tags
   ```

9. Merge the release branch back to develop so that we keep hold of any changes that we made there.
