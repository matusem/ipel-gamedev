import type { DraftLite, GameManifest, PublishToken, UploadGameZipResponse } from "../generated-types/index";

type Variables = Record<string, unknown>;

export class SdkApiClient {
  constructor(public readonly serverUrl: string) {}

  private async gqlRaw(bearer: string, query: string, variables: Variables): Promise<any> {
    const res = await fetch(this.serverUrl, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${bearer}`
      },
      body: JSON.stringify({ query, variables })
    });
    const json = await res.json();
    if (json.errors) {
      throw new Error(`GraphQL error: ${JSON.stringify(json.errors)}`);
    }
    return json.data;
  }

  async createPublishToken(userId: string, ttlDays = 7): Promise<PublishToken> {
    const query = `mutation($ttlDays: Int!) { createPublishToken(ttlDays: $ttlDays) { token userId expiresAt } }`;
    const data = await this.gqlRaw(userId, query, { ttlDays });
    return {
      token: data.createPublishToken.token,
      user_id: data.createPublishToken.userId,
      expires_at: data.createPublishToken.expiresAt
    };
  }

  async uploadGameZipBase64(token: string, filename: string, zipBase64: string): Promise<UploadGameZipResponse> {
    const query = `mutation($filename: String!, $zipBase64: String!) {
      uploadGameZip(filename: $filename, zipBase64: $zipBase64) {
        uploadId
        report { ok errors warnings infos diagnostics { severity code message } }
        draft { id gameName version status }
      }
    }`;
    const data = await this.gqlRaw(token, query, { filename, zipBase64 });
    const u = data.uploadGameZip;
    return {
      upload_id: u.uploadId,
      draft: u.draft
        ? { id: u.draft.id, game_name: u.draft.gameName, version: u.draft.version, status: u.draft.status }
        : null,
      report: u.report
    };
  }

  async listMyDrafts(token: string): Promise<DraftLite[]> {
    const query = `query { myGameDrafts { id gameName version status } }`;
    const data = await this.gqlRaw(token, query, {});
    return data.myGameDrafts.map((d: any) => ({
      id: d.id,
      game_name: d.gameName,
      version: d.version,
      status: d.status
    }));
  }

  async updateDraftManifest(token: string, draftId: string, manifest: GameManifest): Promise<any> {
    const query = `mutation($draftId: ID!, $name: String!, $displayName: String!, $version: String!, $description: String!) {
      updateGameDraftManifest(draftId: $draftId, name: $name, displayName: $displayName, version: $version, description: $description) { id gameName version status }
    }`;
    const data = await this.gqlRaw(token, query, {
      draftId,
      name: manifest.name,
      displayName: manifest.display_name,
      version: manifest.version,
      description: manifest.description
    });
    return data.updateGameDraftManifest;
  }
}
