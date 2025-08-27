# EKS Cluster Configuration for BitCraps

module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 19.0"

  cluster_name    = local.cluster_name
  cluster_version = "1.28"

  cluster_endpoint_public_access  = true
  cluster_endpoint_private_access = true

  cluster_addons = {
    coredns = {
      most_recent = true
    }
    kube-proxy = {
      most_recent = true
    }
    vpc-cni = {
      most_recent = true
    }
    aws-ebs-csi-driver = {
      most_recent = true
    }
  }

  vpc_id                   = module.vpc.vpc_id
  subnet_ids               = module.vpc.private_subnets
  control_plane_subnet_ids = module.vpc.intra_subnets

  # Security groups
  cluster_security_group_additional_rules = {
    egress_nodes_ephemeral_ports_tcp = {
      description                = "To node 1025-65535"
      protocol                   = "tcp"
      from_port                  = 1025
      to_port                    = 65535
      type                       = "egress"
      source_node_security_group = true
    }
  }

  node_security_group_additional_rules = {
    ingress_self_all = {
      description = "Node to node all ports/protocols"
      protocol    = "-1"
      from_port   = 0
      to_port     = 0
      type        = "ingress"
      self        = true
    }
    egress_all = {
      description      = "Node all egress"
      protocol         = "-1"
      from_port        = 0
      to_port          = 0
      type             = "egress"
      cidr_blocks      = ["0.0.0.0/0"]
      ipv6_cidr_blocks = ["::/0"]
    }
  }

  # EKS Managed Node Group(s)
  eks_managed_node_group_defaults = {
    instance_types = var.eks_node_instance_types[var.environment]
    
    # Enable detailed monitoring
    enable_monitoring = true
    
    # Block device mappings
    block_device_mappings = {
      xvda = {
        device_name = "/dev/xvda"
        ebs = {
          volume_size           = var.environment == "prod" ? 100 : 50
          volume_type           = "gp3"
          iops                  = 3000
          throughput            = 125
          encrypted             = true
          delete_on_termination = true
        }
      }
    }
    
    # Labels
    labels = {
      Environment = var.environment
      GithubRepo  = "bitcraps"
    }
    
    tags = local.common_tags
  }

  eks_managed_node_groups = {
    # General compute nodes
    general = {
      name            = "${local.cluster_name}-general"
      use_name_prefix = true

      min_size     = var.eks_node_group_min_size[var.environment]
      max_size     = var.eks_node_group_max_size[var.environment]
      desired_size = var.eks_node_group_desired_size[var.environment]

      instance_types = var.eks_node_instance_types[var.environment]
      capacity_type  = var.environment == "prod" ? "ON_DEMAND" : "SPOT"

      update_config = {
        max_unavailable_percentage = 50
      }

      taints = []

      labels = {
        role = "general"
      }
    }

    # Gateway nodes for internet bridging (production only)
    gateway = {
      create = var.environment == "prod"
      
      name            = "${local.cluster_name}-gateway"
      use_name_prefix = true

      min_size     = var.environment == "prod" ? 2 : 0
      max_size     = var.environment == "prod" ? 4 : 0
      desired_size = var.environment == "prod" ? 2 : 0

      instance_types = ["m5.large"]
      capacity_type  = "ON_DEMAND"

      taints = [
        {
          key    = "workload"
          value  = "gateway"
          effect = "NO_SCHEDULE"
        }
      ]

      labels = {
        role = "gateway"
        workload = "gateway"
      }
    }
  }

  # Enable IRSA
  enable_irsa = true

  # Cluster IAM role
  cluster_iam_role_name            = "${local.cluster_name}-cluster-role"
  cluster_iam_role_use_name_prefix = false
  
  # Node IAM role
  node_iam_role_name            = "${local.cluster_name}-node-role"
  node_iam_role_use_name_prefix = false
  
  node_iam_role_additional_policies = {
    AmazonSSMManagedInstanceCore = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
    CloudWatchAgentServerPolicy  = "arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy"
  }

  tags = merge(
    local.common_tags,
    {
      "karpenter.sh/discovery" = local.cluster_name
    }
  )
}

# OIDC Provider for IRSA
data "tls_certificate" "eks" {
  url = module.eks.cluster_oidc_issuer_url
}

resource "aws_iam_openid_connect_provider" "eks" {
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = [data.tls_certificate.eks.certificates[0].sha1_fingerprint]
  url             = module.eks.cluster_oidc_issuer_url

  tags = local.common_tags
}

# Outputs
output "cluster_endpoint" {
  description = "Endpoint for EKS control plane"
  value       = module.eks.cluster_endpoint
}

output "cluster_security_group_id" {
  description = "Security group ID attached to the EKS cluster"
  value       = module.eks.cluster_security_group_id
}

output "cluster_name" {
  description = "The name of the EKS cluster"
  value       = module.eks.cluster_name
}