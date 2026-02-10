import { reactive } from 'vue';
import { tauriInvoke } from './tauriApi';

interface ConnectionInfo {
  id: string;
  name: string;
  host: string;
  port: number;
  database: string;
  username: string;
  lastUsed: string | null;
  createdAt: string;
}

interface ConnectionState {
  connected: boolean;
  activeConnection: ConnectionInfo | null;
  connections: ConnectionInfo[];
  loading: boolean;
  error: string | null;
}

const state = reactive<ConnectionState>({
  connected: false,
  activeConnection: null,
  connections: [],
  loading: false,
  error: null,
});

export const useDbConnection = () => {
  const connect = async (
    host: string,
    port: number,
    database: string,
    username: string,
    password: string,
    connectionName?: string
  ) => {
    state.loading = true;
    state.error = null;
    try {
      const msg = await tauriInvoke<string>('connect_db', {
        request: { host, port, database, username, password },
      });
      state.connected = true;
      state.activeConnection = {
        id: '',
        name: connectionName || `${host}/${database}`,
        host,
        port,
        database,
        username,
        lastUsed: new Date().toISOString(),
        createdAt: new Date().toISOString(),
      };
      return msg;
    } catch (e) {
      state.error = String(e);
      state.connected = false;
      throw e;
    } finally {
      state.loading = false;
    }
  };

  const connectSaved = async (id: string) => {
    state.loading = true;
    state.error = null;
    try {
      const msg = await tauriInvoke<string>('connect_saved', { id });
      state.connected = true;
      const saved = state.connections.find((c) => c.id === id);
      if (saved) {
        state.activeConnection = saved;
      }
      return msg;
    } catch (e) {
      state.error = String(e);
      state.connected = false;
      throw e;
    } finally {
      state.loading = false;
    }
  };

  const disconnect = async () => {
    try {
      await tauriInvoke('disconnect_db');
    } finally {
      state.connected = false;
      state.activeConnection = null;
    }
  };

  const testConnection = async (
    host: string,
    port: number,
    database: string,
    username: string,
    password: string
  ): Promise<boolean> => {
    try {
      await tauriInvoke('test_connection', {
        request: { host, port, database, username, password },
      });
      return true;
    } catch (e) {
      state.error = String(e);
      return false;
    }
  };

  const loadConnections = async () => {
    try {
      const connections = await tauriInvoke<ConnectionInfo[]>('get_connections');
      state.connections = connections;
    } catch (e) {
      state.error = String(e);
    }
  };

  const saveConnection = async (
    name: string,
    host: string,
    port: number,
    database: string,
    username: string,
    password: string
  ) => {
    try {
      const saved = await tauriInvoke<ConnectionInfo>('save_connection', {
        request: { name, host, port, database, username, password },
      });
      state.connections.push(saved);
      return saved;
    } catch (e) {
      state.error = String(e);
      throw e;
    }
  };

  const deleteConnection = async (id: string) => {
    try {
      await tauriInvoke('delete_connection', { id });
      state.connections = state.connections.filter((c) => c.id !== id);
    } catch (e) {
      state.error = String(e);
      throw e;
    }
  };

  return {
    state,
    connect,
    connectSaved,
    disconnect,
    testConnection,
    loadConnections,
    saveConnection,
    deleteConnection,
  };
};
