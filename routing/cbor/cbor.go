// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Package cbor is a minimal Concise Binary Object Representation (RFC 8949)
// codec, scoped to exactly what the Bundle Protocol version 7 (RFC 9171) wire
// format needs: unsigned/negative integers, byte and text strings, definite-
// and indefinite-length arrays, maps, tags, and the simple values used for
// booleans and null.
//
// It is intentionally small and dependency-free rather than a general-purpose
// CBOR library. Decoding yields a canonical set of Go types so that callers can
// round-trip bundles without reflection:
//
//	unsigned integer  -> uint64
//	negative integer  -> int64
//	byte string       -> []byte
//	text string       -> string
//	array             -> []interface{}
//	map               -> map[interface{}]interface{}
//	tag               -> Tag
//	false/true        -> bool
//	null/undefined    -> nil
//	float             -> float64
package cbor

import (
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"math"
)

// Major types (the high 3 bits of the initial byte).
const (
	majorUint   = 0
	majorNint   = 1
	majorBytes  = 2
	majorText   = 3
	majorArray  = 4
	majorMap    = 5
	majorTag    = 6
	majorSimple = 7
)

// indefinite is the low-5-bit additional-information value denoting an
// indefinite-length item, and, in major type 7, the "break" stop code.
const indefinite = 31

// IndefArray marks a slice to be encoded as a CBOR indefinite-length array
// (major type 4 with a trailing break). RFC 9171 requires each bundle to be
// encoded as an indefinite-length array of blocks.
type IndefArray []interface{}

// Tag is a CBOR tagged value (major type 6): a tag number wrapping one item.
type Tag struct {
	Number  uint64
	Content interface{}
}

// ErrUnsupported is returned when a Go value cannot be encoded, or a CBOR
// construct is not handled by this minimal codec.
var ErrUnsupported = errors.New("cbor: unsupported value")

// Marshal encodes v into a CBOR byte slice.
func Marshal(v interface{}) ([]byte, error) {
	var e encoder
	if err := e.encode(v); err != nil {
		return nil, err
	}
	return e.buf, nil
}

type encoder struct {
	buf []byte
}

func (e *encoder) writeHead(major byte, arg uint64) {
	mt := major << 5
	switch {
	case arg < 24:
		e.buf = append(e.buf, mt|byte(arg))
	case arg < 1<<8:
		e.buf = append(e.buf, mt|24, byte(arg))
	case arg < 1<<16:
		var b [2]byte
		binary.BigEndian.PutUint16(b[:], uint16(arg))
		e.buf = append(e.buf, mt|25)
		e.buf = append(e.buf, b[:]...)
	case arg < 1<<32:
		var b [4]byte
		binary.BigEndian.PutUint32(b[:], uint32(arg))
		e.buf = append(e.buf, mt|26)
		e.buf = append(e.buf, b[:]...)
	default:
		var b [8]byte
		binary.BigEndian.PutUint64(b[:], arg)
		e.buf = append(e.buf, mt|27)
		e.buf = append(e.buf, b[:]...)
	}
}

func (e *encoder) encode(v interface{}) error {
	switch x := v.(type) {
	case nil:
		e.buf = append(e.buf, majorSimple<<5|22) // null
	case bool:
		if x {
			e.buf = append(e.buf, majorSimple<<5|21)
		} else {
			e.buf = append(e.buf, majorSimple<<5|20)
		}
	case uint:
		e.writeHead(majorUint, uint64(x))
	case uint8:
		e.writeHead(majorUint, uint64(x))
	case uint16:
		e.writeHead(majorUint, uint64(x))
	case uint32:
		e.writeHead(majorUint, uint64(x))
	case uint64:
		e.writeHead(majorUint, x)
	case int:
		e.encodeInt(int64(x))
	case int8:
		e.encodeInt(int64(x))
	case int16:
		e.encodeInt(int64(x))
	case int32:
		e.encodeInt(int64(x))
	case int64:
		e.encodeInt(x)
	case float64:
		e.buf = append(e.buf, majorSimple<<5|27)
		var b [8]byte
		binary.BigEndian.PutUint64(b[:], math.Float64bits(x))
		e.buf = append(e.buf, b[:]...)
	case []byte:
		e.writeHead(majorBytes, uint64(len(x)))
		e.buf = append(e.buf, x...)
	case string:
		e.writeHead(majorText, uint64(len(x)))
		e.buf = append(e.buf, x...)
	case []interface{}:
		e.writeHead(majorArray, uint64(len(x)))
		for _, item := range x {
			if err := e.encode(item); err != nil {
				return err
			}
		}
	case IndefArray:
		e.buf = append(e.buf, majorArray<<5|indefinite)
		for _, item := range x {
			if err := e.encode(item); err != nil {
				return err
			}
		}
		e.buf = append(e.buf, majorSimple<<5|indefinite) // break
	case map[interface{}]interface{}:
		e.writeHead(majorMap, uint64(len(x)))
		for k, val := range x {
			if err := e.encode(k); err != nil {
				return err
			}
			if err := e.encode(val); err != nil {
				return err
			}
		}
	case Tag:
		e.writeHead(majorTag, x.Number)
		return e.encode(x.Content)
	default:
		return fmt.Errorf("%w: %T", ErrUnsupported, v)
	}
	return nil
}

func (e *encoder) encodeInt(x int64) {
	if x >= 0 {
		e.writeHead(majorUint, uint64(x))
		return
	}
	e.writeHead(majorNint, uint64(-1-x))
}

// Unmarshal decodes a single CBOR item from the front of data and returns the
// decoded value together with the number of bytes consumed. Trailing bytes are
// left for the caller to inspect.
func Unmarshal(data []byte) (interface{}, int, error) {
	d := decoder{data: data}
	v, err := d.decode()
	if err != nil {
		return nil, d.pos, err
	}
	return v, d.pos, nil
}

type decoder struct {
	data []byte
	pos  int
}

// breakMarker is the internal sentinel returned when a break stop code is read.
type breakMarker struct{}

func (d *decoder) readByte() (byte, error) {
	if d.pos >= len(d.data) {
		return 0, io.ErrUnexpectedEOF
	}
	b := d.data[d.pos]
	d.pos++
	return b, nil
}

func (d *decoder) readN(n int) ([]byte, error) {
	if n < 0 || d.pos+n > len(d.data) {
		return nil, io.ErrUnexpectedEOF
	}
	b := d.data[d.pos : d.pos+n]
	d.pos += n
	return b, nil
}

// readArg reads the argument that follows the initial byte's additional
// information field. It returns the argument value and whether the item is
// indefinite-length.
func (d *decoder) readArg(ai byte) (uint64, bool, error) {
	switch {
	case ai < 24:
		return uint64(ai), false, nil
	case ai == 24:
		b, err := d.readByte()
		return uint64(b), false, err
	case ai == 25:
		b, err := d.readN(2)
		if err != nil {
			return 0, false, err
		}
		return uint64(binary.BigEndian.Uint16(b)), false, nil
	case ai == 26:
		b, err := d.readN(4)
		if err != nil {
			return 0, false, err
		}
		return uint64(binary.BigEndian.Uint32(b)), false, nil
	case ai == 27:
		b, err := d.readN(8)
		if err != nil {
			return 0, false, err
		}
		return binary.BigEndian.Uint64(b), false, nil
	case ai == indefinite:
		return 0, true, nil
	default:
		return 0, false, fmt.Errorf("cbor: reserved additional information %d", ai)
	}
}

func (d *decoder) decode() (interface{}, error) {
	b, err := d.readByte()
	if err != nil {
		return nil, err
	}
	major := b >> 5
	ai := b & 0x1f

	if major == majorSimple {
		return d.decodeSimple(ai)
	}

	arg, indef, err := d.readArg(ai)
	if err != nil {
		return nil, err
	}

	switch major {
	case majorUint:
		return arg, nil
	case majorNint:
		return int64(-1) - int64(arg), nil
	case majorBytes:
		if indef {
			return d.decodeIndefBytes()
		}
		raw, err := d.readN(int(arg))
		if err != nil {
			return nil, err
		}
		out := make([]byte, len(raw))
		copy(out, raw)
		return out, nil
	case majorText:
		if indef {
			return d.decodeIndefText()
		}
		raw, err := d.readN(int(arg))
		if err != nil {
			return nil, err
		}
		return string(raw), nil
	case majorArray:
		return d.decodeArray(arg, indef)
	case majorMap:
		return d.decodeMap(arg, indef)
	case majorTag:
		content, err := d.decode()
		if err != nil {
			return nil, err
		}
		return Tag{Number: arg, Content: content}, nil
	default:
		return nil, fmt.Errorf("cbor: unknown major type %d", major)
	}
}

func (d *decoder) decodeSimple(ai byte) (interface{}, error) {
	switch ai {
	case 20:
		return false, nil
	case 21:
		return true, nil
	case 22, 23:
		return nil, nil
	case indefinite:
		return breakMarker{}, nil
	case 25:
		b, err := d.readN(2)
		if err != nil {
			return nil, err
		}
		return float64(math.Float32frombits(uint32(halfToFloat32bits(binary.BigEndian.Uint16(b))))), nil
	case 26:
		b, err := d.readN(4)
		if err != nil {
			return nil, err
		}
		return float64(math.Float32frombits(binary.BigEndian.Uint32(b))), nil
	case 27:
		b, err := d.readN(8)
		if err != nil {
			return nil, err
		}
		return math.Float64frombits(binary.BigEndian.Uint64(b)), nil
	default:
		return nil, fmt.Errorf("cbor: unsupported simple value %d", ai)
	}
}

func (d *decoder) decodeArray(arg uint64, indef bool) (interface{}, error) {
	out := []interface{}{}
	if indef {
		for {
			v, err := d.decode()
			if err != nil {
				return nil, err
			}
			if _, ok := v.(breakMarker); ok {
				break
			}
			out = append(out, v)
		}
		return out, nil
	}
	for i := uint64(0); i < arg; i++ {
		v, err := d.decode()
		if err != nil {
			return nil, err
		}
		out = append(out, v)
	}
	return out, nil
}

func (d *decoder) decodeMap(arg uint64, indef bool) (interface{}, error) {
	out := map[interface{}]interface{}{}
	if indef {
		for {
			k, err := d.decode()
			if err != nil {
				return nil, err
			}
			if _, ok := k.(breakMarker); ok {
				break
			}
			v, err := d.decode()
			if err != nil {
				return nil, err
			}
			out[k] = v
		}
		return out, nil
	}
	for i := uint64(0); i < arg; i++ {
		k, err := d.decode()
		if err != nil {
			return nil, err
		}
		v, err := d.decode()
		if err != nil {
			return nil, err
		}
		out[k] = v
	}
	return out, nil
}

func (d *decoder) decodeIndefBytes() (interface{}, error) {
	var out []byte
	for {
		v, err := d.decode()
		if err != nil {
			return nil, err
		}
		if _, ok := v.(breakMarker); ok {
			break
		}
		chunk, ok := v.([]byte)
		if !ok {
			return nil, errors.New("cbor: invalid chunk in indefinite byte string")
		}
		out = append(out, chunk...)
	}
	return out, nil
}

func (d *decoder) decodeIndefText() (interface{}, error) {
	var out []byte
	for {
		v, err := d.decode()
		if err != nil {
			return nil, err
		}
		if _, ok := v.(breakMarker); ok {
			break
		}
		chunk, ok := v.(string)
		if !ok {
			return nil, errors.New("cbor: invalid chunk in indefinite text string")
		}
		out = append(out, chunk...)
	}
	return string(out), nil
}

// halfToFloat32bits converts an IEEE 754 half-precision (float16) bit pattern
// to the equivalent single-precision bit pattern.
func halfToFloat32bits(h uint16) uint32 {
	sign := uint32(h&0x8000) << 16
	exp := uint32(h>>10) & 0x1f
	mant := uint32(h & 0x3ff)
	switch exp {
	case 0:
		if mant == 0 {
			return sign
		}
		// Subnormal: normalize.
		e := int32(-1)
		for mant&0x400 == 0 {
			mant <<= 1
			e--
		}
		mant &= 0x3ff
		exp32 := uint32(int32(127-15) + e + 1)
		return sign | exp32<<23 | mant<<13
	case 0x1f:
		return sign | 0xff<<23 | mant<<13
	default:
		return sign | (exp+(127-15))<<23 | mant<<13
	}
}
