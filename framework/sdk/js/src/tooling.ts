import { SdkApiClient } from "./sdk-client";
import type { UploadGameZipResponse } from "../generated-types/index";

export async function deployZipBytes(
  client: SdkApiClient,
  token: string,
  filename: string,
  zipBytes: Uint8Array
): Promise<UploadGameZipResponse> {
  let binary = "";
  for (let i = 0; i < zipBytes.length; i += 1) {
    binary += String.fromCharCode(zipBytes[i]);
  }
  const zipBase64 = btoa(binary);
  return client.uploadGameZipBase64(token, filename, zipBase64);
}
