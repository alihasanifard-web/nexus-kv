// ==============================================================================
// 🚀 NEXUS-KV OFFICIAL GO CLIENT & DRIVER
// Author: Ali Hasanifard
// License: MIT
// ==============================================================================

package nexus

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"strconv"
	"strings"
	"sync"
	"time"
)

var (
	ErrNil           = errors.New("nexus: nil returned")
	ErrClosed        = errors.New("nexus: client is closed")
	ErrProtocol      = errors.New("nexus: protocol error")
	ErrConnection    = errors.New("nexus: connection error")
)

type Options struct {
	DialTimeout  time.Duration
	ReadTimeout  time.Duration
	WriteTimeout time.Duration
}

type Option func(*Options)

func WithTimeout(d time.Duration) Option {
	return func(o *Options) {
		o.DialTimeout = d
		o.ReadTimeout = d
		o.WriteTimeout = d
	}
}

// Client is a thread-safe client for NexusKV.
type Client struct {
	addr    string
	opts    Options
	mu      sync.Mutex
	conn    net.Conn
	reader  *bufio.Reader
	writer  *bufio.Writer
	closed  bool
}

// NewClient dials NexusKV at the given address (e.g. "127.0.0.1:6379").
func NewClient(addr string, optFns ...Option) (*Client, error) {
	opts := Options{
		DialTimeout:  3 * time.Second,
		ReadTimeout:  3 * time.Second,
		WriteTimeout: 3 * time.Second,
	}
	for _, fn := range optFns {
		fn(&opts)
	}

	c := &Client{
		addr: addr,
		opts: opts,
	}

	if err := c.connect(); err != nil {
		return nil, err
	}
	return c, nil
}

func (c *Client) connect() error {
	conn, err := net.DialTimeout("tcp", c.addr, c.opts.DialTimeout)
	if err != nil {
		return fmt.Errorf("%w: %v", ErrConnection, err)
	}
	c.conn = conn
	c.reader = bufio.NewReader(conn)
	c.writer = bufio.NewWriter(conn)
	c.closed = false
	return nil
}

func (c *Client) sendCommand(args ...string) (interface{}, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.closed {
		return nil, ErrClosed
	}

	// Format Redis RESP array: *N\r\n $len\r\narg\r\n ...
	var buf strings.Builder
	buf.WriteString(fmt.Sprintf("*%d\r\n", len(args)))
	for _, a := range args {
		buf.WriteString(fmt.Sprintf("$%d\r\n%s\r\n", len(a), a))
	}

	if c.opts.WriteTimeout > 0 {
		_ = c.conn.SetWriteDeadline(time.Now().Add(c.opts.WriteTimeout))
	}
	if _, err := c.writer.WriteString(buf.String()); err != nil {
		_ = c.conn.Close()
		return nil, fmt.Errorf("%w: failed to write command: %v", ErrConnection, err)
	}
	if err := c.writer.Flush(); err != nil {
		_ = c.conn.Close()
		return nil, fmt.Errorf("%w: failed to flush command: %v", ErrConnection, err)
	}

	if c.opts.ReadTimeout > 0 {
		_ = c.conn.SetReadDeadline(time.Now().Add(c.opts.ReadTimeout))
	}
	return c.readResponse()
}

func (c *Client) readResponse() (interface{}, error) {
	line, err := c.reader.ReadString('\n')
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrConnection, err)
	}
	if len(line) < 3 {
		return nil, ErrProtocol
	}
	line = strings.TrimSuffix(line, "\r\n")

	prefix := line[0]
	payload := line[1:]

	switch prefix {
	case '+': // Simple String
		return payload, nil
	case ':': // Integer
		return strconv.ParseInt(payload, 10, 64)
	case '-': // Error
		return nil, fmt.Errorf("nexus error: %s", payload)
	case '$': // Bulk String
		length, err := strconv.Atoi(payload)
		if err != nil {
			return nil, ErrProtocol
		}
		if length == -1 {
			return nil, ErrNil
		}
		data := make([]byte, length+2) // data + \r\n
		if _, err := io.ReadFull(c.reader, data); err != nil {
			return nil, err
		}
		return string(data[:length]), nil
	case '*': // Array
		count, err := strconv.Atoi(payload)
		if err != nil {
			return nil, ErrProtocol
		}
		if count == -1 {
			return nil, ErrNil
		}
		items := make([]interface{}, count)
		for i := 0; i < count; i++ {
			item, err := c.readResponse()
			if err != nil && !errors.Is(err, ErrNil) {
				return nil, err
			}
			items[i] = item
		}
		return items, nil
	default:
		return nil, fmt.Errorf("%w: unexpected prefix byte %c", ErrProtocol, prefix)
	}
}

// Ping tests cluster reachability.
func (c *Client) Ping(ctx context.Context) (string, error) {
	res, err := c.sendCommand("PING")
	if err != nil {
		return "", err
	}
	return fmt.Sprint(res), nil
}

// Set stores a key-value pair.
func (c *Client) Set(ctx context.Context, key, val string) error {
	res, err := c.sendCommand("SET", key, val)
	if err != nil {
		return err
	}
	if res != "OK" {
		return fmt.Errorf("unexpected set response: %v", res)
	}
	return nil
}

// SetEx stores a key-value pair with an expiration in seconds.
func (c *Client) SetEx(ctx context.Context, key, val string, seconds int) error {
	res, err := c.sendCommand("SET", key, val, "EX", strconv.Itoa(seconds))
	if err != nil {
		return err
	}
	if res != "OK" {
		return fmt.Errorf("unexpected setex response: %v", res)
	}
	return nil
}

// Get retrieves a key. Returns (value, found, error).
func (c *Client) Get(ctx context.Context, key string) (string, bool, error) {
	res, err := c.sendCommand("GET", key)
	if errors.Is(err, ErrNil) {
		return "", false, nil
	}
	if err != nil {
		return "", false, err
	}
	return fmt.Sprint(res), true, nil
}

// Del deletes one key. Returns number of keys removed (0 or 1).
func (c *Client) Del(ctx context.Context, key string) (int64, error) {
	res, err := c.sendCommand("DEL", key)
	if err != nil {
		return 0, err
	}
	if count, ok := res.(int64); ok {
		return count, nil
	}
	return 0, nil
}

// Expire sets a key's timeout in seconds.
func (c *Client) Expire(ctx context.Context, key string, seconds int) (bool, error) {
	res, err := c.sendCommand("EXPIRE", key, strconv.Itoa(seconds))
	if err != nil {
		return false, err
	}
	if count, ok := res.(int64); ok {
		return count == 1, nil
	}
	return false, nil
}

// TTL gets remaining seconds (-2 if non-existent, -1 if no expire).
func (c *Client) TTL(ctx context.Context, key string) (int64, error) {
	res, err := c.sendCommand("TTL", key)
	if err != nil {
		return -2, err
	}
	if count, ok := res.(int64); ok {
		return count, nil
	}
	return -2, nil
}

// Persist removes key expiration.
func (c *Client) Persist(ctx context.Context, key string) (bool, error) {
	res, err := c.sendCommand("PERSIST", key)
	if err != nil {
		return false, err
	}
	if count, ok := res.(int64); ok {
		return count == 1, nil
	}
	return false, nil
}

// Keys returns all keys matching a wildcard pattern.
func (c *Client) Keys(ctx context.Context, pattern string) ([]string, error) {
	res, err := c.sendCommand("KEYS", pattern)
	if err != nil {
		return nil, err
	}
	arr, ok := res.([]interface{})
	if !ok {
		return nil, nil
	}
	out := make([]string, 0, len(arr))
	for _, item := range arr {
		if s, ok := item.(string); ok {
			out = append(out, s)
		}
	}
	return out, nil
}

// Role reports node consensus state (master, slave, candidate).
func (c *Client) Role(ctx context.Context) (string, error) {
	res, err := c.sendCommand("ROLE")
	if err != nil {
		return "", err
	}
	return fmt.Sprint(res), nil
}

// Close gracefully terminates the connection.
func (c *Client) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.closed {
		return nil
	}
	c.closed = true
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}
