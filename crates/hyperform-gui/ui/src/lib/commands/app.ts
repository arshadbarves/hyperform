import { invoke } from "@tauri-apps/api/core";
import type { CatalogData } from "../catalog";

export const fetchModelCatalog = () => invoke<CatalogData>("fetch_model_catalog");

export interface UpdateInfo {
  available: boolean;
  current: string;
  latest?: string;
  url?: string;
  releaseNotes?: string;
}

export interface UpdateProgress {
  phase: "downloading" | "installing" | "ready";
  downloadedBytes: number;
  totalBytes?: number;
  percentage?: number;
}

export const checkForUpdate = (force = false) =>
  invoke<UpdateInfo>("check_for_update", { force });

export const openReleaseUrl = (url: string) => invoke("open_release_url", { url });

export const installUpdate = () => invoke("install_update");
