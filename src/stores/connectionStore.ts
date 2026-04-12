import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { ConnectionConfig, ConnectionInfo } from '../types';

interface ConnectionState {
  connections: ConnectionInfo[];
  isVaultLocked: boolean;
  isPasswordSet: boolean;
  loading: boolean;

  checkVaultStatus: () => Promise<void>;
  setMasterPassword: (password: string) => Promise<void>;
  unlockVault: (password: string) => Promise<boolean>;
  loadConnections: () => Promise<void>;
  saveConnection: (config: ConnectionConfig) => Promise<string>;
  deleteConnection: (id: string) => Promise<void>;
  updateConnection: (config: ConnectionConfig) => Promise<void>;
  getConnection: (id: string) => Promise<ConnectionConfig>;
}

export const useConnectionStore = create<ConnectionState>((set) => ({
  connections: [],
  isVaultLocked: true,
  isPasswordSet: false,
  loading: true,

  checkVaultStatus: async () => {
    try {
      const passwordSet = await invoke('cmd_is_master_password_set') as boolean;
      set({
        isPasswordSet: passwordSet,
        isVaultLocked: passwordSet,
        loading: false,
      });
    } catch {
      set({ loading: false });
    }
  },

  setMasterPassword: async (password: string) => {
    await invoke('cmd_set_master_password', { password });
    set({ isPasswordSet: true, isVaultLocked: false });
  },

  unlockVault: async (password: string): Promise<boolean> => {
    try {
      const result = await invoke('cmd_unlock_vault', { password }) as boolean;
      if (result) {
        set({ isVaultLocked: false });
      }
      return result;
    } catch {
      return false;
    }
  },

  loadConnections: async () => {
    try {
      const connections = await invoke('cmd_get_connections') as ConnectionInfo[];
      set({ connections });
    } catch {
      set({ connections: [] });
    }
  },

  saveConnection: async (config: ConnectionConfig): Promise<string> => {
    const id = await invoke('cmd_save_connection', { config }) as string;
    const connections = await invoke('cmd_get_connections') as ConnectionInfo[];
    set({ connections });
    return id;
  },

  deleteConnection: async (id: string) => {
    await invoke('cmd_delete_connection', { id });
    set((state) => ({
      connections: state.connections.filter((c) => c.id !== id),
    }));
  },

  updateConnection: async (config: ConnectionConfig) => {
    await invoke('cmd_update_connection', { config });
    const connections = await invoke('cmd_get_connections') as ConnectionInfo[];
    set({ connections });
  },

  getConnection: async (id: string): Promise<ConnectionConfig> => {
    const config = await invoke('cmd_get_connection', { id }) as ConnectionConfig;
    return config;
  },
}));
