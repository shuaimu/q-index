// Live Reload for Development
// This script auto-refreshes the browser when the server restarts

(function() {
    if (window.location.hostname !== 'localhost' && window.location.hostname !== '127.0.0.1') {
        return; // Only run in development
    }

    let lastCheck = Date.now();
    let checkInterval = 1000; // Check every second
    let failCount = 0;

    async function checkServer() {
        try {
            const response = await fetch('/api/stats', {
                method: 'HEAD',
                cache: 'no-cache'
            });
            
            if (response.ok) {
                // Server is up
                if (failCount > 0) {
                    // Server came back online, refresh the page
                    console.log('🔄 Server is back online, refreshing...');
                    window.location.reload();
                }
                failCount = 0;
            }
        } catch (error) {
            // Server is down
            failCount++;
            if (failCount === 1) {
                console.log('🔴 Server went offline, waiting for restart...');
                showReloadNotification();
            }
        }
    }

    function showReloadNotification() {
        const notification = document.createElement('div');
        notification.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            background: #f39c12;
            color: white;
            padding: 15px 20px;
            border-radius: 5px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.2);
            z-index: 10000;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            display: flex;
            align-items: center;
            gap: 10px;
        `;
        notification.innerHTML = `
            <div class="spinner-border spinner-border-sm text-light" role="status">
                <span class="visually-hidden">Loading...</span>
            </div>
            <span>Server restarting... Will auto-refresh when ready.</span>
        `;
        document.body.appendChild(notification);
    }

    // Start checking
    setInterval(checkServer, checkInterval);
    
    // Also listen for WebSocket if available (future enhancement)
    if (window.WebSocket) {
        try {
            const ws = new WebSocket('ws://localhost:8080/ws');
            ws.onmessage = (event) => {
                if (event.data === 'reload') {
                    window.location.reload();
                }
            };
            ws.onerror = () => {
                // Fallback to polling
            };
        } catch (e) {
            // WebSocket not available, use polling
        }
    }

    console.log('📡 Live reload enabled for development');
})();