import { writable, derived, get } from 'svelte/store';
import type { TransferStatus } from '../types';
import { sftpService } from './sftpService';
import { localFsService } from './localFsService';

// Store for active transfers
const activeTransfers = writable<TransferStatus[]>([]);
const queue = writable<TransferStatus[]>([]);
const maxConcurrentTransfers = 3; // Configurable

// Export stores
export const transfers = derived([activeTransfers, queue], ([$active, $queue]) => ({
  active: $active,
  queue: $queue,
  all: [...$active, ...$queue]
}));

export const isTransferring = derived(activeTransfers, $active => $active.length > 0);

export class TransferQueueService {
  private transferMetrics = new Map<string, { lastProgress: number; lastTime: number; bytesTransferred: number }>();
  
  /**
   * Add a new transfer to the queue
   */
  async addTransfer(
    type: 'upload' | 'download',
    sessionId: string,
    localPath: string,
    remotePath: string,
    size: number
  ): Promise<string> {
    const id = crypto.randomUUID();
    const transfer: TransferStatus = {
      id,
      type,
      sessionId,
      localPath,
      remotePath,
      progress: 0,
      speed: 0,
      status: 'pending',
      startTime: new Date(),
      totalSize: size,
      bytesTransferred: 0
    };
    // Initialize metrics for speed calculation
    this.transferMetrics.set(id, {
      lastProgress: 0,
      lastTime: Date.now(),
      bytesTransferred: 0
    });

    queue.update(q => [...q, transfer]);
    this.processQueue();
    
    return id;
  }

  /**
   * Remove a transfer from queue or active
   */
  removeTransfer(id: string): void {
    activeTransfers.update(active => active.filter(t => t.id !== id));
    queue.update(q => q.filter(t => t.id !== id));
    // Clean up metrics
    this.transferMetrics.delete(id);
  }

  /**
   * Cancel a transfer
   */
  cancelTransfer(id: string): void {
    activeTransfers.update(active =>
      active.map(t => t.id === id ? { ...t, status: 'canceled' as const, endTime: new Date() } : t)
    );
    queue.update(q =>
      q.map(t => t.id === id ? { ...t, status: 'canceled' as const, endTime: new Date() } : t)
    );
  }

  /**
   * Pause an active transfer
   */
  pauseTransfer(id: string): void {
    activeTransfers.update(active =>
      active.map(t => t.id === id ? { ...t, status: 'paused' as const } : t)
    );
    queue.update(q =>
      q.map(t => t.id === id ? { ...t, status: 'paused' as const } : t)
    );
  }

  /**
   * Resume a paused transfer
   */
  resumeTransfer(id: string): void {
    // Change status to pending so it gets picked up by queue processor
    activeTransfers.update(active =>
      active.map(t => t.id === id ? { ...t, status: 'pending' as const } : t)
    );
    queue.update(q =>
      q.map(t => t.id === id ? { ...t, status: 'pending' as const } : t)
    );
    // Trigger queue processing
    this.processQueue();
  }

  /**
   * Clear all pending transfers
   */
  clearQueue(): void {
    queue.set([]);
  }

  /**
   * Update transfer progress
   */
  updateProgress(id: string, progress: number, speed: number = 0): void {
    const calculatedSpeed = speed;
    const now = Date.now();
    const metrics = this.transferMetrics.get(id);
    
    if (metrics && progress > metrics.lastProgress) {
      // Calculate speed based on progress delta and time delta
      // Assuming total size is unknown, we estimate based on progress percentage
      // For simplicity, we compute bytes per second from progress percentage change
      const timeDelta = now - metrics.lastTime;
      if (timeDelta > 0) {
        // Estimate bytes transferred based on progress (requires total size)
        // Since we don't have total size, we'll just store progress and compute speed
        // as progress percentage per second (not very useful).
        // Instead, we'll rely on external speed measurement.
        // For now, keep external speed.
      }
    }
    
    // Update metrics
    this.transferMetrics.set(id, {
      lastProgress: progress,
      lastTime: now,
      bytesTransferred: metrics ? metrics.bytesTransferred : 0
    });
    
    activeTransfers.update(active =>
      active.map(t => t.id === id ? { ...t, progress, speed: calculatedSpeed } : t)
    );
  }

  /**
   * Mark transfer as completed
   */
  completeTransfer(id: string): void {
    activeTransfers.update(active =>
      active.map(t => t.id === id ? { ...t, progress: 100, status: 'completed' as const, endTime: new Date() } : t)
    );
    // Clean up metrics
    this.transferMetrics.delete(id);
    // Remove from active after a delay
    setTimeout(() => {
      activeTransfers.update(active => active.filter(t => t.id !== id));
    }, 2000);
  }

  /**
   * Mark transfer as failed
   */
  failTransfer(id: string, error: string): void {
    activeTransfers.update(active =>
      active.map(t => t.id === id ? { ...t, status: 'failed' as const, error, endTime: new Date() } : t)
    );
    queue.update(q => q.filter(t => t.id !== id));
    // Clean up metrics
    this.transferMetrics.delete(id);
  }

  /**
   * Internal: Process the queue
   */
  private processQueue(): void {
    activeTransfers.update(active => {
      if (active.length >= maxConcurrentTransfers) return active;
      
      queue.update(q => {
        // Filter out paused transfers from queue
        const pendingTransfers = q.filter(t => t.status !== 'paused');
        const availableSlots = maxConcurrentTransfers - active.length;
        const toStart = pendingTransfers.slice(0, availableSlots);
        
        // Start these transfers
        toStart.forEach(transfer => {
          // Update status to transferring
          const updated: TransferStatus = {
            ...transfer,
            status: 'transferring' as const
          };
          active = [...active, updated];
          
          // Execute the transfer asynchronously
          this.executeTransfer(updated);
        });
        
        // Remove started transfers from queue (by id)
        const startedIds = new Set(toStart.map(t => t.id));
        return q.filter(t => !startedIds.has(t.id));
      });
      
      return active;
    });
  }

  /**
   * Execute a transfer (upload or download)
   */
  private async executeTransfer(transfer: TransferStatus): Promise<void> {
    const { id, type, sessionId, localPath, remotePath } = transfer;
    
    try {
      if (type === 'upload') {
        // Read local file
        const content = await localFsService.readFile(localPath);
        const totalSize = content.length;
        
        // Determine starting offset for resume
        const metrics = this.transferMetrics.get(id);
        const bytesTransferred = metrics?.bytesTransferred || 0;
        const startOffset = bytesTransferred;
        
        // Update total size if not set
        if (transfer.totalSize === undefined) {
          this.updateTransferField(id, 'totalSize', totalSize);
        }
        
        // Simulate progress updates with pause support
        const chunkSize = 64 * 1024; // 64KB chunks
        try {
          for (let offset = startOffset; offset < totalSize; offset += chunkSize) {
            // Check if transfer is paused or canceled
            const currentStatus = this.getTransferStatus(id);
            if (currentStatus === 'paused' || currentStatus === 'canceled') {
              // Transfer was paused or canceled, exit loop
              return;
            }
            
            const end = Math.min(offset + chunkSize, totalSize);
            const chunk = content.slice(offset, end);
            
            // Write chunk with append flag (append=true for resume)
            await sftpService.writeFile(sessionId, remotePath, chunk, offset > 0);
            
            // Update progress
            const progress = Math.floor((end / totalSize) * 100);
            const speed = chunkSize * 5; // placeholder speed calculation
            this.updateProgress(id, progress, speed);
            
            // Update bytes transferred in metrics
            this.transferMetrics.set(id, {
              lastProgress: progress,
              lastTime: Date.now(),
              bytesTransferred: end
            });
            
            // Small delay to simulate network transfer
            await new Promise(resolve => setTimeout(resolve, 50));
          }
        } catch (e) {
          const currentStatus = this.getTransferStatus(id);
          if (currentStatus === 'paused' || currentStatus === 'canceled') {
            return;
          }
          await sftpService.scpUpload(sessionId, remotePath, content);
          this.updateProgress(id, 100, 0);
          this.transferMetrics.set(id, {
            lastProgress: 100,
            lastTime: Date.now(),
            bytesTransferred: totalSize
          });
        }
      } else {
        // Download from remote via SFTP
        // For now, simulate with pause support
        const totalSize = transfer.totalSize || 1024 * 1024; // default 1MB
        const metrics = this.transferMetrics.get(id);
        const bytesTransferred = metrics?.bytesTransferred || 0;
        const startOffset = bytesTransferred;
        
        const chunkSize = 64 * 1024;
        for (let offset = startOffset; offset < totalSize; offset += chunkSize) {
          // Check if transfer is paused or canceled
          const currentStatus = this.getTransferStatus(id);
          if (currentStatus === 'paused' || currentStatus === 'canceled') {
            return;
          }
          
          const end = Math.min(offset + chunkSize, totalSize);
          // Simulate reading chunk (would need SFTP read with offset)
          // For now, just update progress
          const progress = Math.floor((end / totalSize) * 100);
          const speed = chunkSize * 5;
          this.updateProgress(id, progress, speed);
          
          // Update bytes transferred
          this.transferMetrics.set(id, {
            lastProgress: progress,
            lastTime: Date.now(),
            bytesTransferred: end
          });
          
          await new Promise(resolve => setTimeout(resolve, 50));
        }
        
        // After simulation, actually read the file (full read for now)
        let content: Uint8Array;
        try {
          content = await sftpService.readFile(sessionId, remotePath);
        } catch (e) {
          content = await sftpService.scpDownload(sessionId, remotePath);
        }
        await localFsService.writeFile(localPath, content, false);
      }
      
      this.completeTransfer(id);
    } catch (error: any) {
      this.failTransfer(id, error.message);
    }
  }

  /**
   * Get current status of a transfer by id
   */
  private getTransferStatus(id: string): TransferStatus['status'] | null {
    const active = get(activeTransfers);
    const queued = get(queue);
    const transfer = [...active, ...queued].find(t => t.id === id);
    return transfer ? transfer.status : null;
  }

  /**
   * Update a specific field of a transfer in either active or queue store
   */
  private updateTransferField(id: string, field: keyof TransferStatus, value: any): void {
    // Update in active transfers
    activeTransfers.update(active =>
      active.map(t => t.id === id ? { ...t, [field]: value } : t)
    );
    // Update in queue
    queue.update(q =>
      q.map(t => t.id === id ? { ...t, [field]: value } : t)
    );
  }

  /**
   * Get statistics
   */
  getStats() {
    return {
      active: 0,
      queued: 0,
      completed: 0,
      failed: 0,
      totalSize: 0,
      averageSpeed: 0
    };
  }
}

// Derived store for total speed of all active transfers
export const totalSpeed = derived(activeTransfers, $active => {
  return $active.reduce((sum, transfer) => sum + transfer.speed, 0);
});

// Helper function to format speed (bytes/sec) to human readable string
export function formatSpeed(bytesPerSecond: number): string {
  if (bytesPerSecond === 0) return '0 B/s';
  const k = 1024;
  const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
  const i = Math.floor(Math.log(bytesPerSecond) / Math.log(k));
  return parseFloat((bytesPerSecond / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

export const transferQueueService = new TransferQueueService();
