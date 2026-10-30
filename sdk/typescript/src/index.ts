// ==============================================================================
// 🚀 NEXUS-KV OFFICIAL TYPESCRIPT & JAVASCRIPT CLIENT
// Author: Ali Hasanifard
// License: MIT
// ==============================================================================

import * as net from 'node:net';

export interface NexusClientOptions {
  host?: string;
  port?: number;
  timeout?: number;
  autoReconnect?: boolean;
}

export class NexusError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'NexusError';
  }
}

export class NexusClient {
  private host: string;
  private port: number;
  private timeout: number;
  private socket: net.Socket | null = null;
  private connected: boolean = false;
  private responseQueue: Array<{
    resolve: (val: any) => void;
    reject: (err: any) => void;
  }> = [];
  private buffer: Buffer = Buffer.alloc(0);

  constructor(options: NexusClientOptions = {}) {
    this.host = options.host || '127.0.0.1';
    this.port = options.port || 6379;
    this.timeout = options.timeout || 5000;
  }

  public async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.socket = new net.Socket();
      this.socket.setTimeout(this.timeout);

      this.socket.connect(this.port, this.host, () => {
        this.connected = true;
        resolve();
      });

      this.socket.on('data', (data: Buffer) => {
        this.buffer = Buffer.concat([this.buffer, data]);
        this.processBuffer();
      });

      this.socket.on('error', (err) => {
        if (!this.connected) {
          reject(new NexusError(`Connection failed to ${this.host}:${this.port}: ${err.message}`));
        }
      });

      this.socket.on('close', () => {
        this.connected = false;
        while (this.responseQueue.length > 0) {
          const item = this.responseQueue.shift();
          item?.reject(new NexusError('Connection closed by peer'));
        }
      });
    });
  }

  private processBuffer(): void {
    while (this.buffer.length > 0 && this.responseQueue.length > 0) {
      const parsed = this.parseResp(this.buffer);
      if (parsed === null) {
        break; // Incomplete frame
      }
      this.buffer = this.buffer.subarray(parsed.bytesConsumed);
      const req = this.responseQueue.shift();
      if (parsed.isError) {
        req?.reject(new NexusError(parsed.value));
      } else {
        req?.resolve(parsed.value);
      }
    }
  }

  private parseResp(buf: Buffer): { value: any; bytesConsumed: number; isError: boolean } | null {
    if (buf.length < 3) return null;
    const prefix = String.fromCharCode(buf[0]);
    const newlineIdx = buf.indexOf('\r\n');
    if (newlineIdx === -1) return null;

    const line = buf.toString('utf-8', 1, newlineIdx);

    switch (prefix) {
      case '+': // Simple string
        return { value: line, bytesConsumed: newlineIdx + 2, isError: false };
      case ':': // Integer
        return { value: parseInt(line, 10), bytesConsumed: newlineIdx + 2, isError: false };
      case '-': // Error
        return { value: line, bytesConsumed: newlineIdx + 2, isError: true };
      case '$': { // Bulk string
        const length = parseInt(line, 10);
        if (length === -1) {
          return { value: null, bytesConsumed: newlineIdx + 2, isError: false };
        }
        const dataStart = newlineIdx + 2;
        const dataEnd = dataStart + length;
        if (buf.length < dataEnd + 2) return null;
        const val = buf.toString('utf-8', dataStart, dataEnd);
        return { value: val, bytesConsumed: dataEnd + 2, isError: false };
      }
      case '*': { // Array
        const count = parseInt(line, 10);
        if (count === -1) {
          return { value: null, bytesConsumed: newlineIdx + 2, isError: false };
        }
        let offset = newlineIdx + 2;
        const items = [];
        for (let i = 0; i < count; i++) {
          const item = this.parseResp(buf.subarray(offset));
          if (item === null) return null;
          items.push(item.value);
          offset += item.bytesConsumed;
        }
        return { value: items, bytesConsumed: offset, isError: false };
      }
      default:
        throw new NexusError(`Unexpected RESP prefix: ${prefix}`);
    }
  }

  public async sendCommand(...args: string[]): Promise<any> {
    if (!this.connected || !this.socket) {
      await this.connect();
    }

    let payload = `*${args.length}\r\n`;
    for (const a of args) {
      const b = Buffer.from(a, 'utf-8');
      payload += `$${b.length}\r\n${a}\r\n`;
    }

    return new Promise((resolve, reject) => {
      this.responseQueue.push({ resolve, reject });
      this.socket!.write(payload);
    });
  }

  public async ping(): Promise<string> {
    return this.sendCommand('PING');
  }

  public async set(key: string, value: string): Promise<boolean> {
    const res = await this.sendCommand('SET', key, value);
    return res === 'OK';
  }

  public async setEx(key: string, value: string, seconds: number): Promise<boolean> {
    const res = await this.sendCommand('SET', key, value, 'EX', seconds.toString());
    return res === 'OK';
  }

  public async get(key: string): Promise<string | null> {
    return this.sendCommand('GET', key);
  }

  public async del(key: string): Promise<number> {
    return this.sendCommand('DEL', key);
  }

  public async expire(key: string, seconds: number): Promise<boolean> {
    const res = await this.sendCommand('EXPIRE', key, seconds.toString());
    return res === 1;
  }

  public async ttl(key: string): Promise<number> {
    return this.sendCommand('TTL', key);
  }

  public async persist(key: string): Promise<boolean> {
    const res = await this.sendCommand('PERSIST', key);
    return res === 1;
  }

  public async keys(pattern: string = '*'): Promise<string[]> {
    const res = await this.sendCommand('KEYS', pattern);
    return Array.isArray(res) ? res : [];
  }

  public async scan(cursor: number = 0, pattern: string = '*'): Promise<[number, string[]]> {
    const res = await this.sendCommand('SCAN', cursor.toString(), pattern);
    if (Array.isArray(res) && res.length === 2) {
      return [parseInt(res[0], 10), res[1]];
    }
    return [0, []];
  }

  public async role(): Promise<string> {
    return this.sendCommand('ROLE');
  }

  public async metrics(): Promise<string> {
    return this.sendCommand('METRICS');
  }

  public async flushAll(): Promise<boolean> {
    const res = await this.sendCommand('FLUSHALL');
    return res === 'OK';
  }

  public close(): void {
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
      this.connected = false;
    }
  }
}

export default NexusClient;
