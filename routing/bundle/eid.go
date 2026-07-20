// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package bundle

import (
	"fmt"
	"strconv"
	"strings"
)

// URI scheme code points registered by RFC 9171 for endpoint identifiers.
const (
	SchemeDTN uint64 = 1
	SchemeIPN uint64 = 2
)

// EndpointID is a Bundle Protocol endpoint identifier (RFC 9171 section 4.2.5.1).
//
// Two schemes are supported:
//
//   - dtn  (scheme code 1): a URI of the form "dtn://node/service", plus the
//     distinguished null endpoint "dtn:none".
//   - ipn  (scheme code 2): a URI of the form "ipn:node.service" where node and
//     service are unsigned integers.
type EndpointID struct {
	Scheme uint64

	// SSP holds the scheme-specific part for the dtn scheme (for example
	// "//node1/inbox"). It is empty for the null endpoint.
	SSP string
	// IsNull marks the distinguished null endpoint "dtn:none".
	IsNull bool

	// Node and Service carry the ipn-scheme components.
	Node    uint64
	Service uint64
}

// NullEndpoint is the distinguished "dtn:none" endpoint used, for example, as
// the report-to endpoint when no reporting is desired.
func NullEndpoint() EndpointID {
	return EndpointID{Scheme: SchemeDTN, IsNull: true}
}

// DTN builds a dtn-scheme endpoint from an SSP such as "//node1/inbox".
func DTN(ssp string) EndpointID {
	return EndpointID{Scheme: SchemeDTN, SSP: ssp}
}

// IPN builds an ipn-scheme endpoint from a node and service number.
func IPN(node, service uint64) EndpointID {
	return EndpointID{Scheme: SchemeIPN, Node: node, Service: service}
}

// ParseEID parses an endpoint identifier URI.
func ParseEID(uri string) (EndpointID, error) {
	switch {
	case uri == "dtn:none":
		return NullEndpoint(), nil
	case strings.HasPrefix(uri, "dtn:"):
		ssp := strings.TrimPrefix(uri, "dtn:")
		if ssp == "" {
			return EndpointID{}, fmt.Errorf("bundle: empty dtn ssp in %q", uri)
		}
		return EndpointID{Scheme: SchemeDTN, SSP: ssp}, nil
	case strings.HasPrefix(uri, "ipn:"):
		rest := strings.TrimPrefix(uri, "ipn:")
		parts := strings.SplitN(rest, ".", 2)
		if len(parts) != 2 {
			return EndpointID{}, fmt.Errorf("bundle: ipn eid %q must be node.service", uri)
		}
		node, err := strconv.ParseUint(parts[0], 10, 64)
		if err != nil {
			return EndpointID{}, fmt.Errorf("bundle: ipn node in %q: %w", uri, err)
		}
		service, err := strconv.ParseUint(parts[1], 10, 64)
		if err != nil {
			return EndpointID{}, fmt.Errorf("bundle: ipn service in %q: %w", uri, err)
		}
		return EndpointID{Scheme: SchemeIPN, Node: node, Service: service}, nil
	default:
		return EndpointID{}, fmt.Errorf("bundle: unsupported eid scheme in %q", uri)
	}
}

// MustParseEID is ParseEID that panics on error; convenient for tests and
// static configuration.
func MustParseEID(uri string) EndpointID {
	eid, err := ParseEID(uri)
	if err != nil {
		panic(err)
	}
	return eid
}

// String renders the endpoint identifier back to its URI form.
func (e EndpointID) String() string {
	switch e.Scheme {
	case SchemeDTN:
		if e.IsNull {
			return "dtn:none"
		}
		return "dtn:" + e.SSP
	case SchemeIPN:
		return fmt.Sprintf("ipn:%d.%d", e.Node, e.Service)
	default:
		return fmt.Sprintf("unknown-scheme-%d", e.Scheme)
	}
}

// NodeID returns a canonical identifier for the administrative node that owns
// this endpoint, stripping any service/demux component. Bundles addressed to
// different services on the same node share a NodeID, which is what the routing
// layer keys contacts on.
func (e EndpointID) NodeID() string {
	switch e.Scheme {
	case SchemeDTN:
		if e.IsNull {
			return "dtn:none"
		}
		// SSP is typically "//node/service"; keep "dtn://node".
		trimmed := strings.TrimPrefix(e.SSP, "//")
		node := trimmed
		if i := strings.IndexByte(trimmed, '/'); i >= 0 {
			node = trimmed[:i]
		}
		return "dtn://" + node
	case SchemeIPN:
		return fmt.Sprintf("ipn:%d", e.Node)
	default:
		return e.String()
	}
}

// toCBOR renders the endpoint identifier as the 2-element CBOR array form
// [scheme, ssp] described by RFC 9171.
func (e EndpointID) toCBOR() []interface{} {
	switch e.Scheme {
	case SchemeDTN:
		if e.IsNull {
			return []interface{}{SchemeDTN, uint64(0)}
		}
		return []interface{}{SchemeDTN, e.SSP}
	case SchemeIPN:
		return []interface{}{SchemeIPN, []interface{}{e.Node, e.Service}}
	default:
		return []interface{}{e.Scheme, e.SSP}
	}
}

// eidFromCBOR reconstructs an endpoint identifier from its decoded CBOR form.
func eidFromCBOR(v interface{}) (EndpointID, error) {
	arr, ok := v.([]interface{})
	if !ok || len(arr) != 2 {
		return EndpointID{}, fmt.Errorf("bundle: eid must be a 2-element array, got %T", v)
	}
	scheme, err := asUint64(arr[0])
	if err != nil {
		return EndpointID{}, fmt.Errorf("bundle: eid scheme: %w", err)
	}
	switch scheme {
	case SchemeDTN:
		if n, err := asUint64(arr[1]); err == nil && n == 0 {
			return NullEndpoint(), nil
		}
		ssp, ok := arr[1].(string)
		if !ok {
			return EndpointID{}, fmt.Errorf("bundle: dtn eid ssp must be text, got %T", arr[1])
		}
		return EndpointID{Scheme: SchemeDTN, SSP: ssp}, nil
	case SchemeIPN:
		pair, ok := arr[1].([]interface{})
		if !ok || len(pair) != 2 {
			return EndpointID{}, fmt.Errorf("bundle: ipn eid ssp must be a 2-element array")
		}
		node, err := asUint64(pair[0])
		if err != nil {
			return EndpointID{}, fmt.Errorf("bundle: ipn node: %w", err)
		}
		service, err := asUint64(pair[1])
		if err != nil {
			return EndpointID{}, fmt.Errorf("bundle: ipn service: %w", err)
		}
		return EndpointID{Scheme: SchemeIPN, Node: node, Service: service}, nil
	default:
		return EndpointID{}, fmt.Errorf("bundle: unsupported eid scheme %d", scheme)
	}
}
