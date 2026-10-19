export async function tagReleaseBody(github, repo, tag, expectedCommit) {
  const { data: ref } = await github.rest.git.getRef({ ...repo, ref: `tags/${tag}` });
  if (ref.object.type !== 'tag') {
    throw new Error('Release requires an annotated tag with a nonempty message');
  }
  const { data: annotation } = await github.rest.git.getTag({ ...repo, tag_sha: ref.object.sha });
  const body = annotation.message;
  if (typeof body !== 'string' || !body.trim()) {
    throw new Error('Release requires an annotated tag with a nonempty message');
  }
  let object = annotation.object;
  while (object.type === 'tag') {
    const { data } = await github.rest.git.getTag({ ...repo, tag_sha: object.sha });
    object = data.object;
  }
  if (object.type !== 'commit' || object.sha !== expectedCommit) {
    throw new Error('Tag changed while building; refusing to upload an older revision');
  }
  return body;
}
